use super::catalog;
#[cfg(unix)]
use super::filesystem::resolve_inside;
use super::filesystem::{atomic_write, atomic_write_batch};
use super::initialize::{init, init_from, project_id};
use crate::config::{FlowConfig, ServiceConfig};
use crate::test_support::TestWorkspace;
use std::fs;
use std::path::{Path, PathBuf};

const PRESETS: &[&str] = &[
    "generic",
    "rust-api",
    "angular-only",
    "angular-rust-postgres",
];
fn external() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/preset-compat")
}

#[test]
fn generic_and_frozen_external_presets_are_valid() {
    for name in PRESETS {
        let preset = catalog::find(name, Some(&external())).unwrap();
        if *name == "angular-rust-postgres" {
            assert!(FlowConfig::from_source(&preset.flow).is_err());
            assert!(init_from(&TestWorkspace::new(name), name, false, Some(&external())).is_err());
            continue;
        }
        FlowConfig::from_source(&preset.flow)
            .unwrap_or_else(|error| panic!("preset {name} is invalid: {error:#}"));
    }
}

#[test]
fn project_names_become_portable_ids() {
    assert_eq!(project_id(Path::new("/tmp/My New_API")), "my-new-api");
    assert_eq!(project_id(Path::new("/tmp/123")), "project-123");
}

#[test]
fn init_writes_required_security_configs() {
    let root = TestWorkspace::new("preset-init");

    init(&root, "generic", false).expect("initialize preset");
    let project = crate::project::Project::discover(Some(root.root.clone()), None)
        .expect("discover initialized project");

    assert!(project.audit_config.is_file());
    assert!(project.secrets_config.is_file());
}

#[cfg(unix)]
#[test]
fn init_protects_only_the_harness_gate_state_directory() {
    use std::os::unix::fs::PermissionsExt;

    let root = TestWorkspace::new("preset-private-state");
    fs::set_permissions(&root.root, fs::Permissions::from_mode(0o755)).unwrap();
    let state = root.join(".harness-gate");
    fs::create_dir(&state).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap();
    let nested = state.join("keep");
    fs::create_dir(&nested).unwrap();
    fs::set_permissions(&nested, fs::Permissions::from_mode(0o755)).unwrap();
    let other = root.join("other");
    fs::create_dir(&other).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o755)).unwrap();

    init(&root, "generic", false).expect("initialize existing project");
    assert_eq!(
        fs::metadata(&state).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for path in [&root.root, &other, &nested] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o755,
            "{} must not be modified",
            path.display()
        );
    }

    let fresh = TestWorkspace::new("preset-private-fresh");
    init(&fresh, "generic", false).expect("initialize fresh project");
    assert_eq!(
        fs::metadata(fresh.join(".harness-gate"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[cfg(unix)]
#[test]
fn existing_symlink_cannot_escape_project() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("preset-path");
    let outside = TestWorkspace::new("preset-outside");
    let outside_file = outside.root.join("outside");
    fs::write(&outside_file, "outside").expect("create outside fixture");
    let link = root.root.join("flow.toml");
    symlink(&outside_file, &link).expect("create symlink fixture");

    let result = resolve_inside(&root.root.canonicalize().expect("canonical root"), link);

    assert!(result.is_err());
}

#[test]
fn atomic_write_replaces_complete_content() {
    let root = TestWorkspace::new("preset-write");
    let path = root.root.join("flow.toml");
    fs::write(&path, "old").expect("write old fixture");

    atomic_write(&path, b"new content").expect("replace fixture");

    assert_eq!(
        fs::read_to_string(&path).expect("read fixture"),
        "new content"
    );
}

#[test]
fn atomic_write_batch_writes_all_entries() {
    let root = TestWorkspace::new("preset-batch-write");
    let first = root.root.join(".harness-gate/first");
    let second = root.root.join(".harness-gate/second");

    atomic_write_batch(&[(first.as_path(), b"first"), (second.as_path(), b"second")])
        .expect("write batch");

    assert_eq!(fs::read_to_string(first).expect("read first"), "first");
    assert_eq!(fs::read_to_string(second).expect("read second"), "second");
}

#[test]
fn atomic_write_batch_cleans_staged_files_when_staging_fails() {
    let root = TestWorkspace::new("preset-batch-failure");
    let existing = root.root.join("existing");
    fs::write(&existing, "old").expect("write old fixture");

    let invalid = Path::new("");
    assert!(atomic_write_batch(&[(existing.as_path(), b"new"), (invalid, b"invalid"),]).is_err());

    assert_eq!(fs::read_to_string(existing).expect("read existing"), "old");
    assert!(fs::read_dir(&root.root)
        .expect("read workspace")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .contains("harness-gate-batch")));
}

#[cfg(unix)]
#[test]
fn atomic_write_batch_restores_backups_when_commit_fails() {
    let root = TestWorkspace::new("preset-batch-rollback");
    let existing = root.root.join("existing");
    fs::write(&existing, "old").expect("write old fixture");
    let directory_entry = root.root.join(".");

    assert!(atomic_write_batch(&[
        (existing.as_path(), b"new"),
        (directory_entry.as_path(), b"invalid"),
    ])
    .is_err());

    assert_eq!(fs::read_to_string(existing).expect("read existing"), "old");
    assert!(fs::read_dir(&root.root)
        .expect("read workspace")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .contains("harness-gate-backup")));
}

#[cfg(unix)]
#[test]
fn atomic_write_batch_rejects_broken_symlink() {
    use std::os::unix::fs::symlink;

    let root = TestWorkspace::new("preset-batch-symlink");
    let path = root.root.join("config");
    symlink(root.root.join("missing-target"), &path).expect("create broken symlink");

    assert!(atomic_write_batch(&[(path.as_path(), b"content")]).is_err());
    assert!(fs::symlink_metadata(path)
        .expect("inspect link")
        .file_type()
        .is_symlink());
}

#[test]
fn generated_reference_presets_cross_validate_quality_and_profile_boundaries() {
    use crate::config::quality::{Assurance, QualityConfig};
    for name in PRESETS {
        let preset = catalog::find(name, Some(&external())).unwrap();
        let root = TestWorkspace::new(name);
        if *name == "angular-rust-postgres" {
            assert!(init_from(&root, name, false, Some(&external())).is_err());
            continue;
        }
        init_from(&root, name, false, Some(&external())).unwrap();
        let project = crate::project::Project::discover(Some(root.root.clone()), None).unwrap();
        let quality = QualityConfig::load_optional(&root, &project.config).unwrap();
        if preset.recipe.is_none() {
            assert!(quality.is_none());
            continue;
        }
        let quality = quality.unwrap();
        assert_eq!(quality.project.name, project.config.project.name);
        assert_eq!(quality.profiles["hook"].assurance, Assurance::Partial);
        assert!(quality.profiles["hook"].collectors.is_empty());
        for profile in ["full", "ci"] {
            assert_eq!(quality.profiles[profile].assurance, Assurance::Complete);
            assert_eq!(
                quality.profiles[profile].collectors.len(),
                quality.collectors.len()
            );
            assert_eq!(
                quality.profiles[profile].policies.len(),
                quality.policies.len()
            );
            let mut invalid = quality.clone();
            invalid
                .profiles
                .get_mut(profile)
                .unwrap()
                .collectors
                .clear();
            fs::write(
                root.join(".harness-gate/quality.toml"),
                toml::to_string_pretty(&invalid).unwrap(),
            )
            .unwrap();
            assert!(QualityConfig::load_optional(&root, &project.config).is_err());
        }
        if *name == "angular-rust-postgres" {
            assert_eq!(
                quality
                    .components
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["backend", "frontend"]
            );
            assert_eq!(quality.relationships.len(), 1);
            match project.config.services.get("test-postgres") {
                Some(ServiceConfig::Docker {
                    external_env,
                    inject_env,
                    container_port,
                    healthcheck,
                    connection,
                    ..
                }) => {
                    assert_eq!(external_env.as_deref(), Some("TEST_DATABASE_URL"));
                    assert_eq!(inject_env, "TEST_DATABASE_URL");

                    assert_eq!(*container_port, 5432);
                    assert_eq!(healthcheck.first().map(String::as_str), Some("pg_isready"));
                    assert!(connection.starts_with("postgres://"), "{connection}");
                }
                other => panic!("unexpected legacy database service: {other:?}"),
            }
            let contract = &quality.collectors["frontend-api"];
            assert_eq!(contract.produces.len(), 3);
            assert!(contract
                .produces
                .iter()
                .all(|e| serde_json::to_value(&e.target).unwrap()["kind"] == "relationship"));
        }
    }
}

#[test]
fn unknown_ecosystem_pack_composes_without_catalog_or_core_changes() {
    use crate::config::quality::QualityConfig;
    let root = TestWorkspace::new("nebula-unregistered-2049");
    init(&root, "generic", false).unwrap();
    let project = crate::project::Project::discover(Some(root.root.clone()), None).unwrap();
    let recipe = r#"[{"pack":"nebula-unregistered-2049","bindings":{"binding":"project","source_root":"sources"}}]"#;
    let packs = [(
        "nebula-unregistered-2049",
        include_str!("../../fixtures/workflow/nebula-unregistered.pack.json"),
    )];
    let mut composed =
        super::composition::compose(recipe, &packs, &project.config.project.name).unwrap();
    // The flow-only generic template does not advertise ci; the caller explicitly
    // selects the existing profiles without a technology-specific migration.
    composed.quality.profiles.remove("ci");
    for (path, content) in composed.files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fs::write(
        root.join(".harness-gate/quality.toml"),
        toml::to_string_pretty(&composed.quality).unwrap(),
    )
    .unwrap();
    QualityConfig::load_optional(&root, &project.config)
        .unwrap()
        .unwrap();
    // Re-initializing generic must not erase an explicit ecosystem adoption.
    let quality_path = root.join(".harness-gate/quality.toml");
    let original = fs::read(&quality_path).unwrap();
    init(&root, "generic", true).unwrap();
    assert_eq!(fs::read(&quality_path).unwrap(), original);
    assert!(super::composition::compose(recipe, &[], "project").is_err());
    assert!(super::composition::compose(
        &recipe.replace("source_root", "missing"),
        &packs,
        "project"
    )
    .is_err());
    let duplicate = format!(
        "[{},{}]",
        &recipe[1..recipe.len() - 1],
        &recipe[1..recipe.len() - 1]
    );
    assert!(super::composition::compose(&duplicate, &packs, "project").is_err());
    assert!(super::composition::compose(
        &recipe.replace("project", "../escape"),
        &packs,
        "project"
    )
    .is_err());
}

#[test]
fn composition_rejects_conflicts_and_malformed_bindings() {
    let recipe = r#"[{"pack":"future","bindings":{"left":"same","right":"same"}}]"#;
    for source in [
        r#"{"quality":{"version":2},"files":{}}"#,
        r#"{"quality":{"${left}":{},"${right}":{}},"files":{}}"#,
        r#"{"quality":{"project":{"name":"${left"}},"files":{}}"#,
        r#"{"quality":{},"files":{"/absolute.json":{}}}"#,
        r#"{"quality":{},"files":{".harness-gate/packs/../escape.json":{}}}"#,
        r#"{"quality":{},"files":{},"unexpected":true}"#,
    ] {
        assert!(super::composition::compose(recipe, &[("future", source)], "project").is_err());
    }
}

#[test]
fn quality_support_file_conflict_does_not_partially_initialize_project() {
    let root = TestWorkspace::new("preset-quality-conflict");
    let path = root.join(".harness-gate/packs/app/policy.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "existing policy").unwrap();
    assert!(init_from(&root, "rust-api", false, Some(&external())).is_err());
    assert!(!root.join(".harness-gate/flow.toml").exists());
    assert!(!root.join(".harness-gate/quality.toml").exists());
    assert_eq!(fs::read_to_string(path).unwrap(), "existing policy");
}

fn custom_catalog(root: &Path, entry: serde_json::Value) {
    fs::write(
        root.join("catalog.json"),
        serde_json::json!({
            "schema": "harness-preset-catalog/v1", "presets": {"future": entry}, "packs": {}
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        root.join("flow.toml"),
        include_str!("../../presets/generic.flow.toml"),
    )
    .unwrap();
}

#[test]
fn arbitrary_external_preset_uses_plain_files_without_core_catalog_changes() {
    let source = TestWorkspace::new("external-source");
    custom_catalog(
        &source,
        serde_json::json!({"description":"Future ecosystem", "flow":"flow.toml"}),
    );
    let project = TestWorkspace::new("external-project");
    init_from(&project, "future", false, Some(&source)).unwrap();
    let loaded = crate::project::Project::discover(Some(project.root.clone()), None).unwrap();
    assert!(!loaded.config.steps.is_empty());
    assert!(!project.join(".harness-gate/quality.toml").exists());
    catalog::print_presets(Some(&source)).unwrap();
    catalog::print_presets(None).unwrap();
    assert!(init(&TestWorkspace::new("no-fallback"), "rust-api", false).is_err());
    assert!(catalog::find("absent", Some(&source)).is_err());
    let path = source.join("catalog.json");
    let catalog = fs::read_to_string(&path)
        .unwrap()
        .replace("future", "future-2049");
    fs::write(path, catalog).unwrap();
    assert!(catalog::find("future-2049", Some(&source)).is_ok());
}

#[test]
fn invalid_external_catalogs_fail_before_configuration_publication() {
    let source = TestWorkspace::new("invalid-catalog");
    let project = TestWorkspace::new("invalid-catalog-project");
    let entry = serde_json::json!({"description":"Future", "flow":"flow.toml"});
    custom_catalog(&source, entry);
    let path = source.join("catalog.json");
    let original = fs::read_to_string(&path).unwrap();
    for invalid in [
        original.replace("harness-preset-catalog/v1", "harness-preset-catalog/v99"),
        original.replace("\"future\"", "\"generic\""),
        original.replace("\"future\"", "\"../escape\""),
        original.replace("\"future\"", "\"\""),
        original.replace("\"future\"", "\"future_name\""),
        original.replace("future", &"a".repeat(129)),
        original.replace("\"Future\"", "\"\""),
        original.replace("\"packs\":{}", "\"packs\":{},\"packs\":{}"),
        original.replace("\"packs\":{}", "\"packs\":{\"../escape\":\"pack.json\"}"),
        original.replace("\"packs\":{}", "\"unexpected\":true,\"packs\":{}"),
        "{".into(),
        " ".repeat(1024 * 1024 + 1),
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(init_from(&project, "future", false, Some(&source)).is_err());
        assert!(catalog::print_presets(Some(&source)).is_err());
        assert!(!project.join(".harness-gate/flow.toml").exists());
    }
    let mut oversized: serde_json::Value = serde_json::from_str(&original).unwrap();
    for index in 0..65 {
        oversized["presets"][format!("future-{index}")] =
            serde_json::json!({"description":"Future", "flow":"flow.toml"});
    }
    fs::write(&path, oversized.to_string()).unwrap();
    assert!(catalog::print_presets(Some(&source)).is_err());
    assert!(!project.join(".harness-gate/flow.toml").exists());
    fs::write(&path, original).unwrap();
    assert!(init_from(&project, "generic", false, Some(&source.join("missing"))).is_err());
}

#[test]
fn external_files_reject_escape_missing_non_regular_and_oversized_inputs() {
    let source = TestWorkspace::new("external-inputs");
    let project = TestWorkspace::new("external-input-project");
    for flow in [
        "../escape",
        "/absolute",
        "C:\\escape",
        "./flow.toml",
        "missing",
        "directory",
        "",
    ] {
        custom_catalog(
            &source,
            serde_json::json!({"description":"Future", "flow":flow}),
        );
        fs::create_dir_all(source.join("directory")).unwrap();
        assert!(
            init_from(&project, "future", false, Some(&source)).is_err(),
            "{flow}"
        );
    }
    custom_catalog(
        &source,
        serde_json::json!({"description":"Future", "flow":"flow.toml", "recipe":"missing.json"}),
    );
    assert!(init_from(&project, "future", false, Some(&source)).is_err());
    custom_catalog(
        &source,
        serde_json::json!({"description":"Future", "flow":"flow.toml"}),
    );
    fs::write(source.join("flow.toml"), " ".repeat(1024 * 1024 + 1)).unwrap();
    assert!(init_from(&project, "future", false, Some(&source)).is_err());
    assert!(!project.join(".harness-gate/flow.toml").exists());
}

#[cfg(unix)]
#[test]
fn external_catalog_rejects_symlink_files_and_directories() {
    use std::os::unix::fs::symlink;
    let source = TestWorkspace::new("linked-catalog");
    let outside = TestWorkspace::new("outside-catalog");
    fs::write(
        outside.join("flow.toml"),
        include_str!("../../presets/generic.flow.toml"),
    )
    .unwrap();
    symlink(outside.join("flow.toml"), source.join("linked.toml")).unwrap();
    symlink(&outside.root, source.join("linked-dir")).unwrap();
    for flow in ["linked.toml", "linked-dir/flow.toml"] {
        custom_catalog(
            &source,
            serde_json::json!({"description":"Future", "flow":flow}),
        );
        assert!(catalog::find("future", Some(&source)).is_err());
    }
}

#[test]
fn named_external_preset_keeps_flow_and_quality_project_identity_equal() {
    let root = TestWorkspace::new("named-preset");
    super::initialize::init_named(
        &root,
        "rust-api",
        false,
        Some(&external()),
        Some("my.custom_id"),
    )
    .unwrap();
    let project = crate::project::Project::discover(Some(root.root.clone()), None).unwrap();
    let quality = crate::config::quality::QualityConfig::load_optional(&root, &project.config)
        .unwrap()
        .unwrap();
    assert_eq!(project.config.project.name, "my.custom_id");
    assert_eq!(quality.project.name, "my.custom_id");
}

#[test]
fn invalid_composed_quality_identifier_does_not_publish_configuration() {
    let root = TestWorkspace::new("invalid-quality-name");
    let error = super::initialize::init_named(
        &root,
        "rust-api",
        false,
        Some(&external()),
        Some("7.custom_id"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("invalid quality identifier"));
    assert!(!root.join(".harness-gate/flow.toml").exists());
    assert!(!root.join(".harness-gate/quality.toml").exists());
    assert!(!root.join(".harness-gate/secrets.toml").exists());
}

#[test]
fn catalog_recommendations_require_v2_unique_all_regular_markers() {
    let source = TestWorkspace::new("recommendation-catalog");
    let project = TestWorkspace::new("recommendation-target");
    let entry = serde_json::json!({"description":"Project-owned checks", "flow":"flow.toml", "detect_files":["Cargo.toml", "package.json"]});
    custom_catalog(&source, entry);
    let path = source.join("catalog.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(catalog::choices(Some(&source), Some(&project)).is_err());
    value["schema"] = serde_json::json!("harness-preset-catalog/v2");
    fs::write(&path, value.to_string()).unwrap();
    assert!(catalog::choices(Some(&source), Some(&project))
        .unwrap()
        .recommendation
        .is_none());
    fs::write(project.join("Cargo.toml"), "not parsed or executed").unwrap();
    assert!(catalog::choices(Some(&source), Some(&project))
        .unwrap()
        .recommendation
        .is_none());
    fs::create_dir(project.join("package.json")).unwrap();
    assert!(catalog::choices(Some(&source), Some(&project))
        .unwrap()
        .recommendation
        .is_none());
    fs::remove_dir(project.join("package.json")).unwrap();
    fs::write(project.join("package.json"), "not parsed").unwrap();
    let menu = catalog::choices(Some(&source), Some(&project)).unwrap();
    let recommendation = menu.recommendation.unwrap();
    assert_eq!(recommendation.name, "future");
    assert_eq!(recommendation.markers, ["Cargo.toml", "package.json"]);
    assert!(catalog::choices(None, Some(&project))
        .unwrap()
        .recommendation
        .is_none());
    assert!(
        catalog::choices(Some(&source), Some(&project.join("missing")))
            .unwrap()
            .recommendation
            .is_none()
    );
    value["presets"]["another"] = value["presets"]["future"].clone();
    fs::write(&path, value.to_string()).unwrap();
    assert!(catalog::choices(Some(&source), Some(&project))
        .unwrap()
        .recommendation
        .is_none());
    assert!(!project.join(".harness-gate").exists());
}

#[test]
fn detection_metadata_rejects_invalid_or_v1_fields() {
    let source = TestWorkspace::new("invalid-detection");
    for schema in ["harness-preset-catalog/v1", "harness-preset-catalog/v2"] {
        for markers in [
            serde_json::Value::Null,
            serde_json::json!([]),
            serde_json::json!(["Cargo.toml", "Cargo.toml"]),
            serde_json::json!(["../Cargo.toml"]),
            serde_json::json!(["unknown.toml"]),
            serde_json::json!("Cargo.toml"),
            serde_json::json!([false]),
            serde_json::json!([
                "Cargo.toml",
                "package.json",
                "pyproject.toml",
                "tsconfig.json",
                "Cargo.toml"
            ]),
        ] {
            custom_catalog(
                &source,
                serde_json::json!({"description":"Explicit", "flow":"flow.toml", "detect_files":markers}),
            );
            let path = source.join("catalog.json");
            let mut value: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            value["schema"] = serde_json::json!(schema);
            fs::write(path, value.to_string()).unwrap();
            assert!(
                catalog::choices(Some(&source), Some(&source)).is_err(),
                "{schema}: {markers}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn recommendation_does_not_follow_marker_symlinks() {
    let source = TestWorkspace::new("symlink-detection");
    let project = TestWorkspace::new("symlink-project");
    custom_catalog(
        &source,
        serde_json::json!({"description":"Explicit", "flow":"flow.toml", "detect_files":["Cargo.toml"]}),
    );
    let path = source.join("catalog.json");
    let value = fs::read_to_string(&path)
        .unwrap()
        .replace("harness-preset-catalog/v1", "harness-preset-catalog/v2");
    fs::write(path, value).unwrap();
    std::os::unix::fs::symlink(source.join("flow.toml"), project.join("Cargo.toml")).unwrap();
    assert!(catalog::choices(Some(&source), Some(&project))
        .unwrap()
        .recommendation
        .is_none());
}
