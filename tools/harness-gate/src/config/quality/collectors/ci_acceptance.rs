//! Hosted/local acceptance uses the same configured producer for every shape.
//! These are synthetic transport measurements, never production coverage runs.
use super::*;
use std::time::Instant;

const MATRIX: &str = include_str!("../../../../fixtures/workflow/ci-matrix.json");

pub(super) fn configured_fixture(shape: &Value, mode: &str, profile: &str) -> Fixture {
    let mut fixture = Fixture::workflow("pass", true, false);
    let root = fixture.dir.path().to_path_buf();
    let mut config = serde_json::to_value(fixture.config()).unwrap();
    let mut state = serde_json::to_value(&fixture.state).unwrap();
    let component_template = state["components"]["app"].clone();
    let subject_template = state["subjects"]["module"][0].clone();
    let record_template = fixture.payload["response"]["collection"]["evidence"][0].clone();
    let mut series = state["series"]["stargazer"].clone();
    series["collector"]["name"] = shape["producer"].clone();
    series["name"] = shape["series"].clone();
    series["tool"]["name"] = shape["tool"].clone();
    series["runtime"]["name"] = shape["runtime"].clone();
    let mut metrics = BTreeMap::new();
    for component in shape["components"].as_array().unwrap() {
        for capability in component["capabilities"].as_array().unwrap() {
            metrics.insert(
                capability["name"].as_str().unwrap(),
                capability["value"]["type"].clone(),
            );
        }
    }
    series["metrics"] = metrics
        .into_iter()
        .map(|(name, kind)| json!({"name":name,"type":kind}))
        .collect();
    series["id"] = json!(evidence::series_id(&series).unwrap());
    state["series"]["stargazer"] = series.clone();
    for key in ["components", "subjects"] {
        state[key] = json!({});
        config[key] = json!({});
    }
    config["policies"] = json!({});
    let mut records = vec![];
    let mut artifacts = vec![];
    let mut expectations = vec![];
    let mut rules = vec![];
    let mut policy_ids = vec![];
    let mut subject_ids = vec![];
    for component in shape["components"].as_array().unwrap() {
        let id = component["id"].as_str().unwrap();
        let source = format!("{id}/src/source.txt");
        fs::create_dir_all(root.join(format!("{id}/src"))).unwrap();
        fs::copy(root.join("src/lib.rs"), root.join(&source)).unwrap();
        let mut compiled_component = component_template.clone();
        compiled_component["id"] = json!(id);
        compiled_component["path"] = json!(id);
        compiled_component["source_boundaries"][0]["path"] = json!(format!("{id}/src"));
        compiled_component["metadata"] = component["metadata"].clone();
        state["components"][id] = compiled_component;
        config["components"][id] = json!({"flow_components":["app"],"source_roots":[format!("{id}/src")],"artifact_root":"target/evidence"});
        let mut subject = subject_template.clone();
        subject["component"] = json!(id);
        subject["path"] = json!(source);
        subject["id"] =
            json!(harness_gate::quality::project::subject_id("example", &subject).unwrap());
        state["subjects"][id] = json!([subject.clone()]);
        config["subjects"][id] = json!({"component":id,"kind":"module","selection":{"kind":"explicit","paths":[source]}});
        subject_ids.push(id);
        let mut record = record_template.clone();
        record["id"] = json!(id);
        record["component"] = json!(id);
        record["subject"] = subject;
        record["source"]["path"] = json!(source);
        record["series"] = series.clone();
        record["collector"] = series["collector"].clone();
        record["artifacts"][0]["id"] = json!(id);
        record["artifacts"][0]["path"] = json!(format!("{id}.json"));
        record["artifacts"][0]["source"] = record["source"].clone();
        record["capabilities"] = json!([]);
        record["metrics"] = json!([]);
        for capability in component["capabilities"].as_array().unwrap() {
            let name = capability["name"].as_str().unwrap();
            let required = capability["required"] == true;
            let availability = if mode == "required-unavailable" && required {
                "not_collected"
            } else {
                capability["state"].as_str().unwrap()
            };
            record["capabilities"].as_array_mut().unwrap().push(json!({"metric":name,"state":availability,"reason":"configured fixture","artifacts":[id]}));
            if availability == "supported" {
                let value = if mode == "policy-failure" && required {
                    &capability["failure"]
                } else {
                    &capability["value"]
                };
                record["metrics"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"name":name,"value":value,"artifacts":[id]}));
            }
            let expectation = json!({"target":{"kind":"component","id":id},"capability":name,"series":series["id"]});
            expectations.push(expectation.clone());
            if !capability["limit"].is_null() {
                let policy_id = format!("{id}.{name}");
                rules.push(json!({"id":policy_id,"metric":name,"scope":{"kind":"component","component":id},"limit":capability["limit"],"operator":capability["operator"],"required":required,"on_violation":"fail","remediation_classes":["adjust_configuration"]}));
                config["policies"][&policy_id] = json!({"expectation":expectation,"policy_file":".harness-gate/policy.json","rule":policy_id});
                policy_ids.push(policy_id);
            }
        }
        record["status"] = json!(if record["metrics"].as_array().unwrap().is_empty() {
            "unavailable"
        } else {
            "measured"
        });
        artifacts.push(record["artifacts"][0].clone());
        records.push(record);
    }
    state["selection"]["changed_subject"] = json!(subject_ids);
    config["collectors"]["stargazer"]["produces"] = json!(expectations);
    config["profiles"]["full"]["policies"] = json!(policy_ids);
    fixture.state = serde_json::from_value(state).unwrap();
    let config: QualityConfig = serde_json::from_value(config).unwrap();
    fs::write(
        root.join(".harness-gate/quality.toml"),
        toml::to_string(&config).unwrap(),
    )
    .unwrap();
    write(
        &root.join(".harness-gate/policy.json"),
        &json!({"schema":"harness-policy/v1","rules":rules}),
    );
    fixture.payload["response"]["artifacts"] = json!(artifacts);
    fixture.payload["response"]["collection"]["evidence"] = json!(records);
    // A launch ledger outside the artifact root detects even unsuccessful fallback.
    let script = format!("#!/usr/bin/python3\nimport json, sys\nfrom pathlib import Path\np = json.loads({})\nr = json.load(sys.stdin)\nwith (Path(r['input']['workspace_root']) / 'launches').open('a') as f: f.write('launch\\n')\nfor a in p['response']['artifacts']: (Path(r['artifact_root']) / a['path']).write_text(p['raw'])\nprint(json.dumps(p['response']))\n", serde_json::to_string(&serde_json::to_string(&fixture.payload).unwrap()).unwrap());
    fs::write(&fixture.request.adapter.executable, script).unwrap();
    fixture.request.adapter.name = shape["producer"].as_str().unwrap().into();
    fixture.request.adapter.source_digest = format!(
        "{:x}",
        Sha256::digest(fs::read(&fixture.request.adapter.executable).unwrap())
    );
    fixture.pin();
    let inputs = compiler::compile(&root, &fixture.state).unwrap();
    fixture.request.input = input(
        &inputs,
        &fixture.state,
        "stargazer",
        &claims(&config, &fixture.state, "stargazer").unwrap(),
    );
    fixture.select_profile(profile, false);
    fixture
}

fn unified(fixture: &Fixture) -> Value {
    let project =
        crate::project::Project::discover(Some(fixture.dir.path().to_path_buf()), None).unwrap();
    let report = crate::verify::run(
        &project,
        crate::scope::ScopeResult::all(&project),
        &fixture.state.profile,
        false,
    )
    .unwrap();
    serde_json::from_slice(
        &fs::read(Path::new(&report.report_directory).join("test_result.json")).unwrap(),
    )
    .unwrap()
}

fn launch_count(fixture: &Fixture) -> usize {
    fs::read_to_string(fixture.dir.path().join("launches"))
        .unwrap()
        .lines()
        .count()
}

fn retain_file(root: &Path, relative: &str, files: &mut BTreeMap<String, Value>) {
    let bytes = fs::read(root.join(relative)).unwrap();
    files.insert(
        relative.into(),
        json!({"sha256":format!("{:x}", Sha256::digest(&bytes)),"base64":BASE64.encode(bytes)}),
    );
}

#[test]
fn configured_ci_acceptance_retains_parity_reuse_and_negative_evidence() {
    let started = Instant::now();
    let matrix: Value = serde_json::from_str(MATRIX).unwrap();
    let mut cases = vec![];
    for shape in matrix["shapes"].as_array().unwrap() {
        for mode in matrix["modes"].as_array().unwrap() {
            let tick = Instant::now();
            let mode = mode.as_str().unwrap();
            let mut fixture = configured_fixture(shape, mode, matrix["profile"].as_str().unwrap());
            let original = fixture.retain();
            assert_eq!(launch_count(&fixture), 1);
            fs::remove_file(&fixture.request.adapter.executable).unwrap();
            let reused = fixture.collect().unwrap();
            assert_eq!(original.evidence, reused.evidence);
            assert_eq!(original.responses, reused.responses);
            let report = unified(&fixture);
            assert_eq!(
                report["status"],
                if mode == "pass" { "PASS" } else { "FAIL" },
                "{report:#}"
            );
            assert_eq!(report["quality"]["producers"]["stargazer"], "retained");
            let quality = &report["quality"];
            assert_eq!(quality["evidence"], json!(original.evidence));
            assert_eq!(
                quality["project_report"]["components"]
                    .as_object()
                    .unwrap()
                    .len(),
                shape["components"].as_array().unwrap().len()
            );
            for component in shape["components"].as_array().unwrap() {
                let compiled = quality["inputs"]["project"]["components"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["id"] == component["id"])
                    .unwrap();
                assert_eq!(compiled["metadata"], component["metadata"]);
                let record = quality["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|item| item["component"] == component["id"])
                    .unwrap();
                assert_eq!(record["series"]["name"], shape["series"]);
                assert_eq!(record["collector"]["name"], shape["producer"]);
                for capability in component["capabilities"].as_array().unwrap() {
                    assert!(record["capabilities"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["metric"] == capability["name"]));
                }
            }
            direct_equivalent(&report["quality"], fixture.dir.path());
            let mut files = BTreeMap::new();
            for relative in fixture
                .state
                .config_files
                .keys()
                .chain(fixture.state.retained.values().map(|pin| &pin.path))
            {
                retain_file(fixture.dir.path(), relative, &mut files);
            }
            for relative in [
                ".harness-gate/workflow-state.json",
                ".harness-gate/workflow-keys.json",
                "direct-report.json",
            ] {
                retain_file(fixture.dir.path(), relative, &mut files);
            }
            for component in shape["components"].as_array().unwrap() {
                let id = component["id"].as_str().unwrap();
                retain_file(
                    fixture.dir.path(),
                    &format!("{id}/src/source.txt"),
                    &mut files,
                );
                retain_file(
                    fixture.dir.path(),
                    &format!("target/evidence/{id}.json"),
                    &mut files,
                );
            }
            let mut negatives = BTreeMap::new();
            if mode == "pass" {
                let pin = fixture.state.retained["stargazer"].clone();
                let file = fixture.dir.path().join(&pin.path);
                let bytes = fs::read(&file).unwrap();
                for negative in matrix["negative_cases"].as_array().unwrap() {
                    let negative = negative.as_str().unwrap();
                    let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
                    let first = shape["components"][0]["id"].as_str().unwrap();
                    let mut modified_file = None;
                    match negative {
                        "capability" => {
                            envelope["response"]["collection"]["evidence"][0]["series"]["metrics"]
                                [0]["name"] = matrix["unsupported_capability"].clone()
                        }
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
                        "artifact" => {
                            modified_file =
                                Some(fixture.request.artifact_root.join(format!("{first}.json")))
                        }
                        "source" => {
                            modified_file =
                                Some(fixture.dir.path().join(format!("{first}/src/source.txt")))
                        }
                        "digest" | "missing" => {}
                        other => panic!("unknown negative operation: {other}"),
                    }
                    write(&file, &envelope);
                    fixture.state.retained.get_mut("stargazer").unwrap().sha256 =
                        if negative == "digest" {
                            "0".repeat(64)
                        } else {
                            format!("{:x}", Sha256::digest(fs::read(&file).unwrap()))
                        };
                    if negative == "missing" {
                        fs::remove_file(&file).unwrap();
                    }
                    let original_file = modified_file.as_ref().map(|path| fs::read(path).unwrap());
                    if let Some(path) = &modified_file {
                        fs::write(path, "tampered").unwrap();
                    }
                    write(
                        &fixture.dir.path().join(".harness-gate/workflow-state.json"),
                        &fixture.state,
                    );
                    let rejected = unified(&fixture);
                    assert_eq!(rejected["status"], "FAIL", "{negative}: {rejected:#}");
                    assert_eq!(rejected["quality"]["status"], "blocked");
                    assert_eq!(launch_count(&fixture), 1);
                    negatives.insert(negative.to_owned(), rejected);
                    if let Some(path) = modified_file {
                        fs::write(path, original_file.unwrap()).unwrap();
                    }
                    fs::write(&file, &bytes).unwrap();
                    fixture
                        .state
                        .retained
                        .insert("stargazer".into(), pin.clone());
                }
            }
            assert_eq!(launch_count(&fixture), 1);
            cases.push(json!({"shape":shape["id"],"mode":mode,"profile":matrix["profile"],"status":"pass","seconds":tick.elapsed().as_secs_f64(),"producer_launches":1,"reuse_launches":0,"direct_parity":true,"report":report,"files":files,"negatives":negatives}));
        }
    }
    if let Some(output) = std::env::var_os("HARNESS_GATE_CI_ACCEPTANCE") {
        let directory = Path::new(&output);
        fs::create_dir_all(directory).unwrap();
        let identity: BTreeMap<_, _> = [
            "GITHUB_SHA",
            "GITHUB_RUN_ID",
            "GITHUB_RUN_ATTEMPT",
            "RUNNER_OS",
            "RUNNER_ARCH",
        ]
        .into_iter()
        .map(|key| (key, std::env::var(key).unwrap_or_default()))
        .collect();
        write(
            &directory.join("receipt.json"),
            &json!({"schema":"quality-ci-acceptance/v1","matrix_sha256":format!("{:x}",Sha256::digest(MATRIX.as_bytes())),"identity":identity,"seconds":started.elapsed().as_secs_f64(),"cases":cases}),
        );
    }
}
