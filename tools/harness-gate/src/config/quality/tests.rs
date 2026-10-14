use super::{model::*, *};
use crate::test_support::TestWorkspace;
use std::fs;

const QUALITY: &str = include_str!("../../../fixtures/workflow/quality.toml");
const POLICY: &str = include_str!("../../../fixtures/workflow/policy.json");

#[test]
fn arc_admin_quality_validates_with_imported_flow_and_rejects_binding_loss() {
    let root = TestWorkspace::new("arc-admin-quality");
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/dogfood/arc-admin");
    root.child(".harness-gate");
    for binding in ["backend", "frontend", "frontend-api"] {
        let destination = format!(".harness-gate/packs/{binding}");
        root.child(&destination);
        for file in ["policy.json", "capabilities.json"] {
            fs::copy(
                fixture.join(format!("quality/packs/{binding}/{file}")),
                root.join(format!("{destination}/{file}")),
            )
            .unwrap();
        }
    }
    fs::copy(
        fixture.join("quality/quality.toml"),
        root.join(QUALITY_CONFIG_PATH),
    )
    .unwrap();
    // Keep the frozen import untouched. This test validates quality bindings;
    // adapt its removed service policy to an explicit required external check.
    let source = fs::read_to_string(fixture.join("import/flow.toml")).unwrap();
    assert!(FlowConfig::from_source(&source).is_err());
    let source = source.replace("external_value_policy = \"isolated-postgres\"", "");
    let executable = toml::Value::String(
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );
    let source = format!(
        "{source}\n[[service_checks.test-postgres]]\nexecutable = {executable}\nsha256 = \"{}\"\nprotocol = \"gate-service-check/v1\"\nplugin = \"fixture\"\nversion = \"1\"\ntimeout_secs = 15\n",
        "0".repeat(64)
    );
    let flow = FlowConfig::from_source(&source).unwrap();
    let quality = QualityConfig::load_optional(&root, &flow).unwrap().unwrap();
    assert_eq!(quality.profiles.len(), 2);
    assert_eq!(quality.profiles["hook"].assurance, Assurance::Partial);

    for mutation in ["relationship", "series", "required-policy", "baseline"] {
        let mut broken = quality.clone();
        match mutation {
            "relationship" => {
                broken.relationships.get_mut("frontend-api").unwrap().to = Target::Component {
                    id: "missing".into(),
                }
            }
            "series" => {
                broken.collectors.get_mut("backend").unwrap().produces[0].series =
                    format!("measurement-series/v1:{}", "a".repeat(64))
            }
            "required-policy" => {
                broken
                    .profiles
                    .get_mut("full")
                    .unwrap()
                    .policies
                    .remove("backend.risk.crap");
            }
            "baseline" => broken.baseline.provider = BaselineProvider::None,
            _ => unreachable!(),
        }
        assert!(broken.validate(&flow, &root).is_err(), "{mutation}");
    }
}

fn fixture() -> (TestWorkspace, FlowConfig, QualityConfig) {
    let root = TestWorkspace::new("quality-config");
    root.child(".harness-gate");
    fs::write(root.join(".harness-gate/policy.json"), POLICY).unwrap();
    let flow = toml::from_str(include_str!(
        "../../../fixtures/preset-compat/rust-api.flow.toml"
    ))
    .unwrap();
    (root, flow, toml::from_str(QUALITY).unwrap())
}

#[cfg(unix)]
#[test]
fn quality_accepts_aliased_repository_roots_but_rejects_escaping_children() {
    let (root, flow, quality) = fixture();
    let alias_parent = TestWorkspace::new("quality-root-alias");
    let alias = alias_parent.join("repo");
    std::os::unix::fs::symlink(&*root, &alias).unwrap();
    fs::write(root.join(QUALITY_CONFIG_PATH), QUALITY).unwrap();
    quality.validate(&flow, &alias).unwrap();
    assert!(QualityConfig::load_optional(&alias, &flow)
        .unwrap()
        .is_some());

    std::os::unix::fs::symlink(&*alias_parent, root.join("src")).unwrap();
    assert!(quality
        .validate(&flow, &alias)
        .unwrap_err()
        .to_string()
        .contains("escapes"));
    assert!(QualityConfig::load_optional(&alias, &flow).is_err());
}

fn invalid(change: impl FnOnce(&mut QualityConfig), expected: &str) {
    let (root, flow, mut quality) = fixture();
    change(&mut quality);
    let error = quality.validate(&flow, &root).unwrap_err().to_string();
    assert!(error.contains(expected), "expected {expected}: {error}");
}

#[test]
fn quality_validates_and_round_trips_without_enabling_legacy_flow() {
    let (root, flow, quality) = fixture();
    assert!(QualityConfig::load_optional(&root, &flow)
        .unwrap()
        .is_none());
    quality.validate(&flow, &root).unwrap();
    let source = toml::to_string_pretty(&quality).unwrap();
    fs::write(root.join(QUALITY_CONFIG_PATH), source).unwrap();
    assert!(QualityConfig::load_optional(&root, &flow)
        .unwrap()
        .is_some());
    let schema: serde_json::Value = serde_json::from_str(&schema_json().unwrap()).unwrap();
    let committed: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../schema/quality.schema.json")).unwrap();
    assert_eq!(schema, committed);
}

#[test]
fn quality_rejects_authority_at_every_collector_nesting_level() {
    for field in [
        "required",
        "threshold",
        "thresholds",
        "ratchet",
        "aggregate",
        "pass",
        "fail",
        "release_authority",
        "policy",
    ] {
        for pointer in [
            "/collectors/coverage",
            "/collectors/coverage/produces/0",
            "/collectors/coverage/produces/0/target",
        ] {
            let (_, _, quality) = fixture();
            let mut value = serde_json::to_value(quality).unwrap();
            value
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), true.into());
            assert!(
                serde_json::from_value::<QualityConfig>(value).is_err(),
                "{pointer}/{field}"
            );
        }
    }
}

#[test]
fn quality_rejects_unresolved_references_and_boundaries() {
    invalid(|q| q.version = 2, "version");
    invalid(|q| q.project.name = "wrong".into(), "project.name");
    invalid(
        |q| {
            q.components
                .get_mut("app")
                .unwrap()
                .flow_components
                .insert("missing".into())
                .then_some(())
                .unwrap()
        },
        "flow components",
    );
    invalid(
        |q| q.subjects.get_mut("module").unwrap().component = "missing".into(),
        "unknown component",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Explicit {
                paths: vec!["outside/lib.rs".into()],
            }
        },
        "escapes component",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
                collector: "missing".into(),
                query: "functions".into(),
            }
        },
        "unknown discovery",
    );
    invalid(
        |q| {
            q.collectors.get_mut("coverage").unwrap().produces[0].target = Target::Subject {
                id: "missing".into(),
            }
        },
        "unresolved quality target",
    );
    invalid(
        |q| {
            q.policies.get_mut("coverage").unwrap().expectation.target = Target::Relationship {
                id: "missing".into(),
            }
        },
        "unresolved quality target",
    );
    invalid(|q| q.reporting.output = "../escape".into(), "escape");
    invalid(
        |q| q.components.get_mut("app").unwrap().artifact_root = "C:\\escape".into(),
        "portable",
    );
    invalid(
        |q| q.collectors.get_mut("coverage").unwrap().request = "${REQUEST}".into(),
        "literal",
    );
}

#[test]
fn quality_profiles_enforce_single_compatible_required_producer() {
    invalid(
        |q| q.profiles.get_mut("full").unwrap().policies.clear(),
        "omits required policy",
    );
    invalid(
        |q| {
            q.profiles
                .insert("undeclared".into(), q.profiles["full"].clone());
        },
        "not declared",
    );
    invalid(
        |q| {
            q.profiles.get_mut("full").unwrap().collectors.clear();
        },
        "no producer",
    );
    invalid(
        |q| {
            q.profiles
                .get_mut("full")
                .unwrap()
                .collectors
                .insert("missing".into());
        },
        "unknown collector",
    );
    invalid(
        |q| {
            q.profiles
                .get_mut("full")
                .unwrap()
                .policies
                .insert("missing".into());
        },
        "unknown policy",
    );
    invalid(
        |q| {
            q.collectors
                .insert("duplicate".into(), q.collectors["coverage"].clone());
            q.profiles
                .get_mut("full")
                .unwrap()
                .collectors
                .insert("duplicate".into());
        },
        "conflicting authoritative",
    );
    invalid(
        |q| {
            q.collectors.get_mut("coverage").unwrap().produces[0].series =
                format!("measurement-series/v1:{}", "b".repeat(64))
        },
        "incompatible series",
    );
    invalid(
        |q| q.collectors.get_mut("coverage").unwrap().produces[0].series = "invented".into(),
        "measurement series",
    );
    invalid(
        |q| {
            q.collectors.get_mut("coverage").unwrap().produces[0].capability =
                "coverage..line".into()
        },
        "invalid capability",
    );
    invalid(
        |q| {
            q.profiles
                .get_mut("hook")
                .unwrap()
                .policies
                .insert("coverage".into());
        },
        "no producer",
    );
}

#[test]
fn quality_policy_and_baseline_contradictions_fail_closed() {
    invalid(
        |q| q.policies.get_mut("coverage").unwrap().rule = "missing".into(),
        "unknown policy rule",
    );
    invalid(
        |q| q.policies.get_mut("coverage").unwrap().policy_file = "missing.json".into(),
        "missing",
    );
    invalid(
        |q| {
            q.policies
                .get_mut("coverage")
                .unwrap()
                .expectation
                .capability = "risk.crap".into()
        },
        "capability does not match",
    );
    invalid(
        |q| {
            q.policies.get_mut("coverage").unwrap().expectation.target = Target::Subject {
                id: "module".into(),
            }
        },
        "scope does not match",
    );
    invalid(
        |q| q.baseline.provider = BaselineProvider::None,
        "no provider",
    );
    invalid(|q| q.baseline.required = false, "ratchet policy requires");
    invalid(
        |q| {
            q.baseline.provider = BaselineProvider::Git {
                reference: "".into(),
                merge_base: true,
            }
        },
        "Git reference",
    );
    invalid(
        |q| {
            q.baseline.provider = BaselineProvider::RetainedArtifact {
                manifest: "../stale.json".into(),
            }
        },
        "escape",
    );
    let (root, flow, mut q) = fixture();
    q.baseline.provider = BaselineProvider::RetainedArtifact {
        manifest: "target/base-manifest.json".into(),
    };
    q.validate(&flow, &root).unwrap();
    fs::write(
        root.join(".harness-gate/policy.json"),
        POLICY
            .replace("\"required\": true", "\"required\": false")
            .replace("\"deny_regression\": true", "\"deny_regression\": false")
            .replace(
                "\"allow_legacy_debt\": true",
                "\"allow_legacy_debt\": false",
            ),
    )
    .unwrap();
    q.baseline = Baseline {
        required: false,
        provider: BaselineProvider::None,
    };
    q.profiles.get_mut("full").unwrap().collectors.clear();
    q.validate(&flow, &root).unwrap();
}

#[test]
fn quality_relationships_and_discovery_resolve_in_active_profiles() {
    let (root, flow, mut q) = fixture();
    q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
        collector: "coverage".into(),
        query: "modules".into(),
    };
    q.relationships.insert(
        "api".into(),
        Relationship {
            kind: "consumes".into(),
            from: Target::Component { id: "app".into() },
            to: Target::Subject {
                id: "module".into(),
            },
        },
    );
    q.validate(&flow, &root).unwrap();
    q.relationships.get_mut("api").unwrap().to = Target::Subject {
        id: "missing".into(),
    };
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("unresolved"));
    q.relationships.get_mut("api").unwrap().to = Target::Relationship { id: "api".into() };
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("endpoints"));
    q.relationships.clear();
    q.collectors
        .insert("discovery".into(), q.collectors["coverage"].clone());
    q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
        collector: "discovery".into(),
        query: "modules".into(),
    };
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("omits discovery"));
}

#[cfg(unix)]
#[test]
fn quality_broken_links_and_boundary_symlinks_are_not_opt_out() {
    let (root, flow, q) = fixture();
    std::os::unix::fs::symlink("missing.toml", root.join(QUALITY_CONFIG_PATH)).unwrap();
    assert!(QualityConfig::load_optional(&root, &flow).is_err());
    let outside = TestWorkspace::new("quality-outside");
    std::os::unix::fs::symlink(&outside.root, root.join("src")).unwrap();
    assert!(q
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("escapes"));
}

#[test]
fn quality_schema_closes_collector_authority_and_tagged_variants() {
    let schema: serde_json::Value = serde_json::from_str(&schema_json().unwrap()).unwrap();
    let definitions = &schema["definitions"];
    for (name, fields) in [
        ("Collector", vec!["produces", "protocol", "request"]),
        ("Expectation", vec!["capability", "series", "target"]),
        ("PolicyBinding", vec!["expectation", "policy_file", "rule"]),
    ] {
        assert_eq!(definitions[name]["additionalProperties"], false);
        let actual: Vec<_> = definitions[name]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual, fields);
    }
    for name in ["Target", "SubjectSelection", "BaselineProvider"] {
        for variant in definitions[name]["oneOf"].as_array().unwrap() {
            assert_eq!(variant["additionalProperties"], false);
        }
    }
}

#[test]
fn quality_accepts_exact_subject_and_relationship_policy_bindings() {
    let (root, flow, mut q) = fixture();
    for target in [
        Target::Subject {
            id: "module".into(),
        },
        Target::Relationship { id: "api".into() },
    ] {
        q.relationships.insert(
            "api".into(),
            Relationship {
                kind: "consumes".into(),
                from: Target::Component { id: "app".into() },
                to: Target::Subject {
                    id: "module".into(),
                },
            },
        );
        let mut policy: serde_json::Value = serde_json::from_str(POLICY).unwrap();
        let rule = &mut policy["rules"][0];
        rule.as_object_mut().unwrap().remove("ratchet");
        rule["metric"] = "contract.schema_valid".into();
        rule["operator"] = "eq".into();
        rule["limit"] = serde_json::json!({"type": "boolean", "value": true});
        q.policies
            .get_mut("coverage")
            .unwrap()
            .expectation
            .capability = "contract.schema_valid".into();
        q.collectors.get_mut("coverage").unwrap().produces[0].capability =
            "contract.schema_valid".into();
        rule["scope"] = match &target {
            Target::Subject { id } => serde_json::json!({"kind": "subject", "subject": id}),
            Target::Relationship { id } => {
                serde_json::json!({"kind": "relationship", "relationship": id})
            }
            _ => unreachable!(),
        };
        fs::write(root.join(".harness-gate/policy.json"), policy.to_string()).unwrap();
        q.policies.get_mut("coverage").unwrap().expectation.target = target.clone();
        q.collectors.get_mut("coverage").unwrap().produces[0].target = target;
        q.validate(&flow, &root).unwrap();
    }
}

#[test]
fn quality_rejects_malformed_policy_and_empty_intent() {
    let (root, flow, q) = fixture();
    for source in [
        "{}".to_owned(),
        POLICY.replace(
            "\"required\": true",
            "\"required\": true, \"required\": false",
        ),
        POLICY.replace("\"schema\":", "\"unexpected\":"),
    ] {
        fs::write(root.join(".harness-gate/policy.json"), source).unwrap();
        assert!(q.validate(&flow, &root).is_err());
    }
    invalid(|q| q.project.id = "Bad ID".into(), "identifier");
    invalid(|q| q.components.clear(), "must not be empty");
    invalid(
        |q| q.components.get_mut("app").unwrap().source_roots.clear(),
        "source roots",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection =
                SubjectSelection::Explicit { paths: vec![] }
        },
        "explicit paths",
    );
    invalid(
        |q| {
            q.subjects.get_mut("module").unwrap().selection = SubjectSelection::Discovery {
                collector: "coverage".into(),
                query: " ".into(),
            }
        },
        "query",
    );
    invalid(
        |q| q.collectors.get_mut("coverage").unwrap().produces.clear(),
        "must not be empty",
    );
    invalid(
        |q| {
            let c = q.collectors.get_mut("coverage").unwrap();
            c.produces.push(c.produces[0].clone());
        },
        "duplicate",
    );
    invalid(|q| q.profiles.clear(), "must not be empty");
    invalid(|q| q.reporting.formats.clear(), "must not be empty");
}

#[test]
fn unknown_pack_configures_cheap_and_expensive_profiles_without_ecosystem_dispatch() {
    let (root, mut flow, mut quality) = fixture();
    let metadata: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/workflow/collectors/unknown-profiles.json"
    ))
    .unwrap();
    let mut policy: serde_json::Value = serde_json::from_str(POLICY).unwrap();
    let rule = policy["rules"][0].clone();
    quality.collectors.clear();
    quality.policies.clear();
    policy["rules"] = serde_json::json!([]);
    let (_, _, template) = fixture();
    for (name, metric) in metadata["capabilities"].as_object().unwrap() {
        let mut collector = template.collectors["coverage"].clone();
        collector.produces[0].capability = metric.as_str().unwrap().into();
        let mut binding = template.policies["coverage"].clone();
        binding.expectation = collector.produces[0].clone();
        binding.rule = name.clone();
        let mut rule = rule.clone();
        rule["id"] = name.clone().into();
        rule["metric"] = metric.clone();
        rule["operator"] = "le".into();
        rule["limit"] = if name == "cheap" {
            serde_json::json!({"type":"size","value":10,"unit":"bytes"})
        } else {
            serde_json::json!({"type":"rational","numerator":30,"denominator":1})
        };
        policy["rules"].as_array_mut().unwrap().push(rule);
        quality.collectors.insert(name.clone(), collector);
        quality.policies.insert(name.clone(), binding);
    }
    fs::write(
        root.join(".harness-gate/policy.json"),
        serde_json::to_vec(&policy).unwrap(),
    )
    .unwrap();
    quality.profiles = serde_json::from_value(metadata["profiles"].clone()).unwrap();
    flow.steps[0]
        .profiles
        .extend(quality.profiles.keys().cloned());
    quality.validate(&flow, &root).unwrap();
    for profile in ["full", "ci"] {
        assert_eq!(quality.profiles[profile].assurance, Assurance::Complete);
        for id in ["cheap", "expensive"] {
            assert_eq!(
                quality.participation(profile)["policies"][id]["state"],
                "participating"
            );
            let mut omitted = quality.clone();
            omitted
                .profiles
                .get_mut(profile)
                .unwrap()
                .policies
                .remove(id);
            assert!(omitted
                .validate(&flow, &root)
                .unwrap_err()
                .to_string()
                .contains("omits required policy"));
        }
    }
    assert_eq!(
        quality.participation("hook")["policies"]["expensive"]["state"],
        "not_collected"
    );
    assert_eq!(
        quality.participation("custom-fast")["policies"]["cheap"]["state"],
        "not_collected"
    );
    assert_eq!(
        quality.participation("custom-fast")["policies"]["expensive"]["state"],
        "participating"
    );
}

#[test]
fn certified_reference_policy_participates_in_complete_profiles_with_original_limits_and_series() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/workflow/collectors/certified-rust-profiles.json"
    ))
    .unwrap();
    let (root, mut flow, mut config) = fixture();
    flow.steps[0].profiles.insert("ci".into());
    let template = config.policies["coverage"].clone();
    let mut producer = config.collectors["coverage"].clone();
    producer.produces.clear();
    config.policies.clear();
    let mut rules = reference["rules"].as_array().unwrap().clone();
    for rule in &mut rules {
        // The pack maps its native boundary to the configured component. Metric,
        // requiredness, limits and the full measurement series remain untouched.
        rule["scope"] = serde_json::json!({"kind":"component", "component":"app"});
        let mut binding = template.clone();
        binding.rule = rule["id"].as_str().unwrap().into();
        binding.expectation.capability = rule["metric"].as_str().unwrap().into();
        binding.expectation.series = reference["series"]["id"].as_str().unwrap().into();
        producer.produces.push(binding.expectation.clone());
        config.policies.insert(binding.rule.clone(), binding);
    }
    config.collectors = std::collections::BTreeMap::from([("certified".into(), producer)]);
    config.profiles = serde_json::from_value(reference["profiles"].clone()).unwrap();
    fs::write(
        root.join(".harness-gate/policy.json"),
        serde_json::to_vec(&serde_json::json!({"schema":"harness-policy/v1", "rules":rules}))
            .unwrap(),
    )
    .unwrap();
    config.validate(&flow, &root).unwrap();
    for name in ["full", "ci"] {
        assert_eq!(config.profiles[name].assurance, Assurance::Complete);
        for id in config.policies.keys() {
            assert_eq!(
                config.participation(name)["policies"][id]["state"],
                "participating"
            );
            let mut omitted = config.clone();
            omitted.profiles.get_mut(name).unwrap().policies.remove(id);
            assert!(omitted.validate(&flow, &root).is_err());
        }
        let mut missing = config.clone();
        missing.profiles.get_mut(name).unwrap().collectors.clear();
        assert!(missing.validate(&flow, &root).is_err());
    }
}

#[test]
fn artifact_budget_is_optional_positive_and_digest_visible() {
    let (root, flow, original) = fixture();
    assert!(original.limits.is_none());
    let legacy = serde_json::to_value(&original).unwrap();
    assert!(legacy.get("limits").is_none());
    for bytes in [1, 64 * 1024 * 1024, 1024 * 1024 * 1024] {
        let mut configured = original.clone();
        configured.limits = Some(CollectionLimits {
            max_artifact_bytes: bytes,
        });
        configured.validate(&flow, &root).unwrap();
        let encoded = toml::to_string(&configured).unwrap();
        let roundtrip: QualityConfig = toml::from_str(&encoded).unwrap();
        assert_eq!(roundtrip.limits.unwrap().max_artifact_bytes, bytes);
        assert_ne!(serde_json::to_value(&configured).unwrap(), legacy);
    }
    let mut invalid = original;
    invalid.limits = Some(CollectionLimits {
        max_artifact_bytes: 0,
    });
    assert!(invalid
        .validate(&flow, &root)
        .unwrap_err()
        .to_string()
        .contains("must be positive"));
    for invalid in ["-1", "18446744073709551616", "'unlimited'"] {
        assert!(toml::from_str::<QualityConfig>(&format!(
            "{QUALITY}\n[limits]\nmax_artifact_bytes = {invalid}\n"
        ))
        .is_err());
    }
}
