use super::config::{LocalTestDatabasePolicy, PlaceholderConfig, SecretScanner};
use super::matcher::{compile_local_test_policy, is_local_test_database, looks_like_secret};
use super::*;
use crate::test_support::TestWorkspace;
use std::process::Command;

fn scanner() -> SecretScanner {
    SecretScanner::from_source(include_str!("../../presets/default.secrets.toml"))
        .expect("default preset secret scan config")
}

#[test]
fn default_preset_secret_config_is_valid() {
    SecretScanner::from_source(include_str!("../../presets/default.secrets.toml"))
        .expect("default preset secret scan config");
}

#[test]
fn invalid_secret_config_fails_closed() {
    let invalid_capture = r#"
version = 2

[placeholders]
minimum_unique_characters = 4
maximum_nonalphanumeric_characters = 2
prefixes = ["${"]
markers = ["change-me"]
exact = ["password"]

[[rules]]
id = "broken"
kind = "value"
pattern = "no-capture"
capture = 1
minimum_length = 8
"#;
    let empty_rules = r#"
version = 2
rules = []

[placeholders]
minimum_unique_characters = 4
maximum_nonalphanumeric_characters = 2
prefixes = ["${"]
markers = ["change-me"]
exact = ["password"]
"#;

    assert!(SecretScanner::from_source(invalid_capture).is_err());
    assert!(SecretScanner::from_source(empty_rules).is_err());
}

#[test]
fn placeholder_prefixes_are_configurable() {
    let placeholders = PlaceholderConfig {
        minimum_unique_characters: 4,
        maximum_nonalphanumeric_characters: 2,
        markers: Vec::new(),
        exact: Vec::new(),
        prefixes: vec!["ref:".to_string()],
    };

    assert!(!looks_like_secret(
        b"ref:correct-horse-battery-staple",
        12,
        &placeholders
    ));
    assert!(looks_like_secret(
        b"${correct-horse-battery-staple}",
        12,
        &placeholders
    ));
}

#[test]
fn local_test_database_policy_is_configurable() {
    let policy = compile_local_test_policy(
        "postgres",
        LocalTestDatabasePolicy {
            hosts: vec!["db.internal".to_string()],
            database_suffixes: vec!["_sandbox".to_string()],
            require_username_equals_password: false,
        },
    )
    .expect("compile local database policy");

    assert!(is_local_test_database(
        b"app",
        b"different-password",
        b"db.internal",
        b"arc_admin_sandbox",
        &policy
    ));
    assert!(!is_local_test_database(
        b"postgres",
        b"postgres",
        b"localhost",
        b"arc_admin_test",
        &policy
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn staged_scan_uses_the_staged_secret_config() {
    let workspace = TestWorkspace::new("staged-secrets");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize staged scan fixture");
    workspace.init_git();
    assert!(Command::new("git")
        .args(["add", "--", ".harness-gate/secrets.toml"])
        .current_dir(&workspace.root)
        .status()
        .expect("stage secret config")
        .success());
    let project =
        Project::discover(Some(workspace.root.clone()), None).expect("discover Git fixture");
    fs::write(
            &project.secrets_config,
            "version = 2\nrules = []\n[placeholders]\nminimum_unique_characters = 4\nmaximum_nonalphanumeric_characters = 2\nprefixes = []\nmarkers = []\nexact = []\n",
        )
        .expect("replace working-tree secret config");

    assert!(scanner_for_mode(&project, SecretMode::Staged).is_ok());
    assert!(scanner_for_mode(&project, SecretMode::WorkingTree).is_err());
}

#[test]
fn detects_direct_tokens_without_matching_placeholders() {
    let patterns = scanner();
    let github_token = ["token=gh", "p_abcdefghijklmnopqrstuvwxyz123456"].concat();
    let access_key = ["AK", "IAIOSFODNN7EXAMPLE"].concat();
    let jwt = [
        "eyJhbGciOiJIUzI1NiIs",
        "InR5cCI6IkpXVCJ9",
        ".eyJzdWIiOiIxMjM0NTY3ODkwIn0",
        ".SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    ]
    .concat();

    assert!(patterns.is_match(github_token.as_bytes()));
    assert!(patterns.is_match(access_key.as_bytes()));
    assert!(patterns.is_match(jwt.as_bytes()));
    assert!(!patterns.is_match(b"JWT_SECRET=change-me-in-production"));
}

#[test]
fn detects_named_jwt_and_enterprise_messaging_secrets() {
    let patterns = scanner();
    let jwt_secret = ["JWT_", "SECRET=correct-horse-battery-staple"].concat();
    let wecom_key = ["WECOM_WEBHOOK_", "KEY=8fbf86b6-4f96-4b69-a97c-6ec55f845db1"].concat();
    let dingtalk_secret = [
        "\"DINGTALK_APP_",
        "SECRET\": \"SECc65f4f1654f544f9ba2a71eb4d498\"",
    ]
    .concat();

    assert!(patterns.is_match(jwt_secret.as_bytes()));
    assert!(patterns.is_match(wecom_key.as_bytes()));
    assert!(patterns.is_match(dingtalk_secret.as_bytes()));
    assert!(!patterns.is_match(["JWT_SECRET=", "$", "{JWT_SECRET}"].concat().as_bytes()));
    assert!(!patterns.is_match(b"WECOM_WEBHOOK_KEY=replace-me"));
}

#[test]
fn detects_postgres_credentials_and_secret_bearing_webhooks() {
    let patterns = scanner();
    let database_url = [
        "DATABASE_URL=postgresql://app:",
        "m4pL9vQ2sR7x@db.internal/arc_admin",
    ]
    .concat();
    let wecom_url = [
        "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=",
        "8fbf86b6-4f96-4b69-a97c-6ec55f845db1",
    ]
    .concat();
    let dingtalk_url = [
        "WEBHOOK_URL=https://oapi.dingtalk.com/robot/send?access_token=",
        "4a8f57c90e51458a825b82d78948bffd",
    ]
    .concat();
    let generic_webhook = [
        "ALERT_WEBHOOK_URL=https://hooks.internal.example/notify/",
        "c51d43d18e6046e0b4ae192c187a44c7",
    ]
    .concat();

    assert!(patterns.is_match(database_url.as_bytes()));
    assert!(patterns.is_match(wecom_url.as_bytes()));
    assert!(patterns.is_match(dingtalk_url.as_bytes()));
    assert!(patterns.is_match(generic_webhook.as_bytes()));
    assert!(
        !patterns.is_match(b"DATABASE_URL=postgresql://postgres:postgres@localhost/arc_admin_test")
    );
    let remote_test_database = [
        "DATABASE_URL=postgresql://arc_admin_test:",
        "arc_admin_test@db.internal/arc_admin_test",
    ]
    .concat();
    assert!(patterns.is_match(remote_test_database.as_bytes()));
    assert!(!patterns.is_match(
        [
            "WEBHOOK_URL=https://example.com/hooks/",
            "$",
            "{WEBHOOK_TOKEN}"
        ]
        .concat()
        .as_bytes()
    ));
}

#[test]
#[cfg(unix)]
fn working_tree_scan_skips_file_symlinks() {
    use std::os::unix::fs::symlink;

    let workspace = TestWorkspace::new("symlink-secret");
    let outside = TestWorkspace::new("symlink-secret-outside");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    fs::write(
        outside.root.join("secret.txt"),
        "outside-only-secret-marker",
    )
    .expect("write outside secret fixture");
    symlink(
        outside.root.join("secret.txt"),
        workspace.root.join("linked.txt"),
    )
    .expect("create secret symlink");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");

    let findings = scan(&project, SecretMode::WorkingTree).expect("scan working tree");
    assert!(findings.is_empty());
}

#[test]
fn working_tree_scan_rejects_an_oversized_file() {
    let workspace = TestWorkspace::new("large-working-tree-secret");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    fs::File::create(workspace.root.join("large.bin"))
        .expect("create large file")
        .set_len(MAX_SECRET_SCAN_FILE_BYTES + 1)
        .expect("size large file");
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");

    let error = scan(&project, SecretMode::WorkingTree)
        .expect_err("oversized working-tree file must fail closed");

    assert!(error.to_string().contains("large.bin"));
    assert!(error.to_string().contains("is too large"));
}

#[test]
fn staged_scan_rejects_an_oversized_blob_before_loading_it() {
    let workspace = TestWorkspace::new("large-staged-secret");
    crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
    workspace.init_git();
    assert!(Command::new("git")
        .args(["add", "--", ".harness-gate/secrets.toml"])
        .current_dir(&workspace.root)
        .status()
        .expect("stage secret config")
        .success());
    assert!(Command::new("git")
        .args(["commit", "--quiet", "-m", "add secret config"])
        .current_dir(&workspace.root)
        .status()
        .expect("commit secret config")
        .success());
    fs::File::create(workspace.root.join("large.bin"))
        .expect("create large file")
        .set_len(MAX_SECRET_SCAN_FILE_BYTES + 1)
        .expect("size large file");
    assert!(Command::new("git")
        .args(["add", "--", "large.bin"])
        .current_dir(&workspace.root)
        .status()
        .expect("stage large file")
        .success());
    let project = Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");

    let error = scan(&project, SecretMode::Staged)
        .expect_err("oversized staged file must fail before loading");
    let message = error.to_string();

    assert!(message.contains("large.bin"), "{message}");
    assert!(message.contains("is too large"), "{message}");
}
