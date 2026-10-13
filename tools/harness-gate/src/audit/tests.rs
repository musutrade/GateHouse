use super::*;
use crate::test_support::TestWorkspace;
#[cfg(target_os = "linux")]
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

struct TestDir(TestWorkspace);

impl TestDir {
    fn new(name: &str) -> Self {
        Self(TestWorkspace::new(name))
    }

    fn child(&self, name: &str) -> PathBuf {
        self.0.child(name)
    }
}

fn configured_audit() -> Config {
    let mut config = parse_audit_config(include_str!("../../presets/empty.audit.toml"))
        .expect("audit test preset must parse");
    config.arch_rules.push(ArchRule {
        name: "Service 层不应包含 SQL 查询".into(),
        layer: "service".into(),
        paths: vec!["services".into()],
        extensions: vec!["rs".into()],
        forbidden_patterns: vec![
            r"sqlx\s*::\s*(?:query|query_as|query_scalar|raw_sql)!?\s*\(".into(),
            r"(?:sqlx\s*::\s*)?QueryBuilder(?:\s*::<[^>]+>)?\s*::\s*new\s*\(".into(),
        ],
        allowed_patterns: Vec::new(),
        suggestion: "move SQL into a repository".into(),
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    });
    config
}

fn configured_hard_rule(name: &str) -> HardRule {
    match name {
        "日志不得记录完整请求头或显式敏感字段" => HardRule {
            name: name.into(),
            severity: "blocker".into(),
            paths: vec!["src".into()],
            extensions: vec!["rs".into()],
            patterns: vec![
                r"include_headers\s*\(\s*true\s*\)".into(),
                r"(?is)(?:tracing\s*::\s*)?(?:trace|debug|info|warn|error)!\s*\([^;]*?\b(?:authorization|cookie|password|(?:access_|refresh_)?token|database_url)\s*=".into(),
                r"(?is)(?:tracing\s*::\s*)?(?:trace|debug|info|warn|error)!\s*\([^;]*?[?%]\s*(?:authorization|cookie|password|(?:access_|refresh_)?token|database_url)\b".into(),
            ],
            exclude_patterns: Vec::new(),
            allowlist: Vec::new(),
        },
        "SQL 写操作仅允许出现在 Repository/迁移/测试层" => HardRule {
            name: name.into(),
            severity: "blocker".into(),
            paths: vec!["src".into()],
            extensions: vec!["rs".into(), "sql".into()],
            patterns: vec![
                r"(?i)INSERT\s+INTO".into(),
                r#"(?i)UPDATE\s+(?:"[^"]+"|[A-Za-z_][A-Za-z0-9_.]*)\s+SET"#.into(),
                r"(?i)DELETE\s+FROM".into(),
                r"(?:sqlx\s*::\s*)?raw_sql\s*\(".into(),
                r"(?:sqlx\s*::\s*)?QueryBuilder(?:\s*::<[^>]+>)?\s*::\s*new\s*\(".into(),
            ],
            exclude_patterns: Vec::new(),
            allowlist: Vec::new(),
        },
        _ => panic!("unknown audit test rule {name:?}"),
    }
}

#[test]
#[cfg(target_os = "linux")]
fn audit_schema_accepts_current_config_and_rejects_incompatible_inputs() {
    let current = include_str!("../../presets/empty.audit.toml");
    assert_eq!(
        parse_audit_config(current)
            .expect("current audit preset must parse")
            .version,
        AUDIT_CONFIG_VERSION
    );

    let missing_version = current.replacen("version = 2\n", "", 1);
    let error = parse_audit_config(&missing_version)
        .expect_err("an unversioned audit config must fail closed");
    assert!(error.to_string().contains("add `version = 2`"));

    let unknown_version = current.replacen("version = 2", "version = 99", 1);
    let error =
        parse_audit_config(&unknown_version).expect_err("an unknown audit schema must fail closed");
    assert!(error
        .to_string()
        .contains("unsupported audit config schema version 99"));

    let unknown_field = format!("unknown = true\n{current}");
    assert!(parse_audit_config(&unknown_field).is_err());
}

#[test]
fn legacy_audit_shapes_receive_actionable_migration_errors() {
    let missing_engine = "version = 2\nhard_rules = []\narch_rules = []\n";
    let error = parse_audit_config(missing_engine)
        .expect_err("schema v2 requires an explicit engine configuration");
    assert!(error.to_string().contains("requires `[engine]`"));

    let legacy_allowlist = r#"
version = 2

[engine]
ignore_filename = ".auditignore"
json_report_filename = "review_context.json"
markdown_report_filename = "review_context.md"
markdown_max_bytes = 4096
markdown_occurrences_per_rule = 3

[engine.comment_syntax.rs]
line = ["//"]

[[hard_rules]]
name = "legacy"
severity = "error"
paths = ["src"]
extensions = ["rs"]
patterns = ["forbidden"]
allowlist = ["src/generated"]
"#;
    let error = parse_audit_config(legacy_allowlist)
        .expect_err("string allowlists must require an explicit migration");
    assert!(error
        .to_string()
        .contains("no longer accepts string allowlist"));
    assert!(error.to_string().contains("kind = \"path-prefix\""));
}

#[test]
#[cfg(target_os = "linux")]
fn rule_extension_requires_configured_comment_syntax() {
    let test_dir = TestDir::new("missing-comment-syntax");
    let source = test_dir.child("src");
    fs::write(source.join("sample.go"), "package sample\n").expect("write Go fixture");
    let mut config = configured_audit();
    config.hard_rules = vec![HardRule {
        name: "Go rule".into(),
        severity: "error".into(),
        paths: vec!["src".into()],
        extensions: vec!["go".into()],
        patterns: vec!["forbidden".into()],
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    }];
    config.arch_rules.clear();

    let error = validate_audit_config(&test_dir.0, &config)
        .expect_err("an extension without lexical syntax must fail closed");
    assert!(error.to_string().contains("[engine.comment_syntax.go]"));
}

#[test]
#[cfg(target_os = "linux")]
fn initialized_project_can_add_and_run_its_first_audit_rule() {
    let test_dir = TestDir::new("initialized-first-rule");
    crate::preset::init(&test_dir.0, "generic", false).expect("initialize project");
    let source = test_dir.child("src");
    fs::write(
        source.join("sample.rs"),
        "// forbidden_call()\nfn sample() { forbidden_call(); }\n",
    )
    .expect("write Rust fixture");
    let config_path = test_dir.0.join(".harness-gate/audit.toml");
    let mut config = fs::read_to_string(&config_path).expect("read initialized audit config");
    config.push_str(
        r#"

[[hard_rules]]
name = "first rule"
severity = "error"
paths = ["src"]
extensions = ["rs"]
patterns = ["forbidden_call"]
allowlist = []
exclude_patterns = []
"#,
    );
    fs::write(&config_path, config).expect("append first audit rule");

    let outcome = run(
        &test_dir.0,
        &config_path,
        &test_dir.0.join(".harness-gate/reports"),
        false,
    )
    .expect("run first audit rule");

    assert_eq!(outcome.total_violations, 1);
    assert_eq!(outcome.error_count, 1);
}

#[test]
fn hard_rule_scans_every_configured_root() {
    let test_dir = TestDir::new("hard-roots");
    let first = test_dir.child("first");
    let second = test_dir.child("second");
    fs::write(first.join("one.rs"), "forbidden_call();\n").expect("write first fixture");
    fs::write(second.join("two.rs"), "forbidden_call();\n").expect("write second fixture");

    let roots = vec![first, second];
    let rule = HardRule {
        name: "test rule".to_string(),
        severity: "error".to_string(),
        paths: Vec::new(),
        extensions: vec!["rs".to_string()],
        patterns: vec!["forbidden_call".to_string()],
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    };
    let violations = scan_files(&test_dir.0, &roots, &[], &rule, &configured_audit().engine)
        .expect("scan fixture");

    assert_eq!(violations.len(), 2);
}

#[test]
fn hard_rule_detects_multiline_sensitive_logging() {
    let test_dir = TestDir::new("multiline-sensitive-log");
    let source = test_dir.child("src");
    fs::write(
        source.join("leak.rs"),
        concat!(
            "fn leak(password: &str, access_token: &str) {\n",
            "    tracing::error!(\n",
            "        password = password,\n",
            "        \"login failed\"\n",
            "    );\n",
            "    warn!(\n",
            "        ?access_token,\n",
            "        \"request failed\"\n",
            "    );\n",
            "    // tracing::error!(password = password);\n",
            "}\n",
        ),
    )
    .expect("write sensitive log fixture");
    let rule = configured_hard_rule("日志不得记录完整请求头或显式敏感字段");

    let violations = scan_files(
        &test_dir.0,
        &[source],
        &[],
        &rule,
        &configured_audit().engine,
    )
    .expect("scan sensitive log fixture");

    assert_eq!(violations.len(), 2);
    assert_eq!(
        violations
            .iter()
            .map(|violation| violation.line)
            .collect::<Vec<_>>(),
        vec![2, 6]
    );
}

#[test]
fn hard_rule_detects_multiline_and_dynamic_sql_surfaces() {
    let test_dir = TestDir::new("multiline-sql");
    let source = test_dir.child("src");
    fs::write(
        source.join("write.rs"),
        concat!(
            "const SQL: &str = r#\"\n",
            "UPDATE\n",
            "    users\n",
            "SET disabled = true\n",
            "\"#;\n",
        ),
    )
    .expect("write multiline SQL fixture");
    fs::write(
        source.join("raw.rs"),
        "sqlx::\n    raw_sql(\"SELECT 1\");\n// raw_sql(\"SELECT 2\");\n",
    )
    .expect("write raw SQL fixture");
    fs::write(
        source.join("builder.rs"),
        "QueryBuilder::<Postgres>\n    ::new(\"SELECT 1\");\n",
    )
    .expect("write query builder fixture");
    fs::write(
        source.join("write.sql"),
        "-- DELETE FROM ignored\nDELETE\nFROM sessions;\n",
    )
    .expect("write external SQL fixture");
    let rule = configured_hard_rule("SQL 写操作仅允许出现在 Repository/迁移/测试层");

    let violations = scan_files(
        &test_dir.0,
        &[source],
        &[],
        &rule,
        &configured_audit().engine,
    )
    .expect("scan SQL fixtures");

    assert_eq!(violations.len(), 4);
    assert!(violations
        .iter()
        .any(|violation| violation.file.ends_with("write.sql") && violation.line == 2));
}

#[test]
#[cfg(target_os = "linux")]
fn architecture_rule_detects_multiline_raw_sql_and_query_builder() {
    let test_dir = TestDir::new("multiline-service-sql");
    let services = test_dir.child("services");
    fs::write(
        services.join("raw.rs"),
        "sqlx\n    ::\n    raw_sql(\"SELECT 1\");\n",
    )
    .expect("write service raw SQL fixture");
    fs::write(
        services.join("builder.rs"),
        "QueryBuilder::<Postgres>\n    ::new(\"SELECT 1\");\n",
    )
    .expect("write service query builder fixture");
    let mut service_rule = configured_audit()
        .arch_rules
        .into_iter()
        .find(|rule| rule.name == "Service 层不应包含 SQL 查询")
        .expect("service SQL rule must exist");
    service_rule.paths = vec!["services".to_string()];
    let config = Config {
        version: AUDIT_CONFIG_VERSION,
        engine: configured_audit().engine,
        paths: PathsConfig::default(),
        hard_rules: Vec::new(),
        arch_rules: vec![service_rule],
    };

    let violations = scan_arch_rules(&test_dir.0, &config, &[]).expect("scan service SQL fixtures");

    assert_eq!(violations.len(), 2);
}

#[test]
#[cfg(target_os = "linux")]
fn architecture_rule_scans_every_configured_root() {
    let test_dir = TestDir::new("arch-roots");
    let pages = test_dir.child("pages");
    let layout = test_dir.child("layout");
    fs::write(pages.join("page.ts"), "HttpClient\n").expect("write page fixture");
    fs::write(layout.join("layout.ts"), "HttpClient\n").expect("write layout fixture");

    let config = Config {
        version: AUDIT_CONFIG_VERSION,
        engine: configured_audit().engine,
        paths: PathsConfig {
            exclude: Vec::new(),
            aliases: HashMap::new(),
        },
        hard_rules: Vec::new(),
        arch_rules: vec![ArchRule {
            name: "component rule".to_string(),
            layer: "component".to_string(),
            paths: vec!["pages".into(), "layout".into()],
            extensions: vec!["ts".to_string()],
            forbidden_patterns: vec!["HttpClient".to_string()],
            allowed_patterns: Vec::new(),
            suggestion: "use a service".to_string(),
            exclude_patterns: Vec::new(),
            allowlist: Vec::new(),
        }],
    };

    assert_eq!(
        scan_arch_rules(&test_dir.0, &config, &[])
            .expect("scan config")
            .len(),
        2
    );
}

#[test]
fn literal_allowlist_is_a_path_prefix_not_a_substring() {
    let allowlist = vec![AllowlistEntry::PathPrefix {
        path: "backend/src/repositories".to_string(),
    }];
    let root = Path::new("/repo");

    assert!(is_allowlisted(
        Path::new("/repo/backend/src/repositories/users.rs"),
        root,
        &allowlist
    ));
    assert!(!is_allowlisted(
        Path::new("/repo/backend/src/repositories_backup/users.rs"),
        root,
        &allowlist
    ));
}

#[test]
fn regex_allowlist_is_explicit() {
    let allowlist = vec![AllowlistEntry::Regex {
        pattern: r"^backend/src/generated/.*\.rs$".to_string(),
    }];
    let root = Path::new("/repo");

    assert!(is_allowlisted(
        Path::new("/repo/backend/src/generated/users.rs"),
        root,
        &allowlist
    ));
    assert!(!is_allowlisted(
        Path::new("/repo/backend/src/services/users.rs"),
        root,
        &allowlist
    ));
}

#[test]
fn configured_comment_syntax_handles_strings_and_block_comments() {
    let test_dir = TestDir::new("comment-syntax");
    let source = test_dir.child("src");
    fs::write(
        source.join("sample.rs"),
        concat!(
            "fn sample() {\n",
            "    let url = \"https://example.invalid\"; forbidden_call();\n",
            "    /* forbidden_call();\n",
            "       /* forbidden_call(); */\n",
            "    */\n",
            "    let marker = \"/*\"; forbidden_call();\n",
            "    // forbidden_call();\n",
            "}\n",
        ),
    )
    .expect("write comment fixture");
    let rule = HardRule {
        name: "comment rule".into(),
        severity: "error".into(),
        paths: Vec::new(),
        extensions: vec!["rs".into()],
        patterns: vec!["forbidden_call".into()],
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    };

    let violations = scan_files(
        &test_dir.0,
        &[source],
        &[],
        &rule,
        &configured_audit().engine,
    )
    .expect("scan comment fixture");

    assert_eq!(
        violations
            .iter()
            .map(|violation| violation.line)
            .collect::<Vec<_>>(),
        vec![2, 6]
    );
}

#[test]
fn missing_rule_root_fails_closed() {
    let test_dir = TestDir::new("missing-root");
    let config = Config {
        version: AUDIT_CONFIG_VERSION,
        engine: configured_audit().engine,
        paths: PathsConfig::default(),
        hard_rules: vec![HardRule {
            name: "missing".into(),
            severity: "blocker".into(),
            paths: vec!["srrc".into()],
            extensions: vec!["rs".into()],
            patterns: vec!["forbidden".into()],
            exclude_patterns: Vec::new(),
            allowlist: Vec::new(),
        }],
        arch_rules: Vec::new(),
    };

    let error =
        validate_audit_config(&test_dir.0, &config).expect_err("missing audit roots must fail");
    assert!(error.to_string().contains("is missing"));
}

#[test]
fn rule_root_cannot_escape_project() {
    let test_dir = TestDir::new("outside-root");
    let config = Config {
        version: AUDIT_CONFIG_VERSION,
        engine: configured_audit().engine,
        paths: PathsConfig::default(),
        hard_rules: vec![HardRule {
            name: "outside".into(),
            severity: "blocker".into(),
            paths: vec!["../outside".into()],
            extensions: vec!["rs".into()],
            patterns: vec!["forbidden".into()],
            exclude_patterns: Vec::new(),
            allowlist: Vec::new(),
        }],
        arch_rules: Vec::new(),
    };

    let error = validate_audit_config(&test_dir.0, &config)
        .expect_err("audit roots must stay inside project");
    assert!(error.to_string().contains("may not escape"));
}

#[test]
fn invalid_rule_regex_returns_an_error() {
    let error = compile_regexes(&["(".to_string()]).expect_err("invalid regex must fail");
    assert!(error.to_string().contains("invalid audit regex"));
}

#[test]
fn report_keeps_rule_names_with_shared_prefixes_separate() {
    let config = Config {
        version: AUDIT_CONFIG_VERSION,
        engine: configured_audit().engine,
        paths: PathsConfig::default(),
        hard_rules: vec![
            HardRule {
                name: "foo".into(),
                severity: "error".into(),
                paths: vec!["src".into()],
                extensions: vec!["rs".into()],
                patterns: vec!["foo".into()],
                exclude_patterns: Vec::new(),
                allowlist: Vec::new(),
            },
            HardRule {
                name: "foobar".into(),
                severity: "blocker".into(),
                paths: vec!["src".into()],
                extensions: vec!["rs".into()],
                patterns: vec!["bar".into()],
                exclude_patterns: Vec::new(),
                allowlist: Vec::new(),
            },
        ],
        arch_rules: Vec::new(),
    };
    let violations = vec![
        Violation {
            file: PathBuf::from("src/foo.rs"),
            line: 1,
            content: "foo".into(),
            rule_name: "foo".into(),
        },
        Violation {
            file: PathBuf::from("src/bar.rs"),
            line: 1,
            content: "bar".into(),
            rule_name: "foobar".into(),
        },
    ];

    let report = super::report::generate_report(&config, &violations, &[]);
    assert_eq!(report.summary.total_violations, 2);
    assert_eq!(report.hard_violations[0].count, 1);
    assert_eq!(report.hard_violations[1].count, 1);
}

#[test]
fn audit_reports_redact_violation_content_in_json_and_markdown() {
    let mut config = configured_audit();
    config.hard_rules = vec![HardRule {
        name: "credential rule".into(),
        severity: "error".into(),
        paths: Vec::new(),
        extensions: vec!["rs".into()],
        patterns: vec!["token".into()],
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    }];
    config.arch_rules.clear();
    let secret = "Authorization: Bearer audit-secret-token";
    let violations = vec![Violation {
        file: PathBuf::from("src/token.rs"),
        line: 7,
        content: secret.into(),
        rule_name: "credential rule".into(),
    }];

    let report = super::report::generate_report(&config, &violations, &[]);
    let json = serde_json::to_string(&report).expect("serialize redacted audit report");
    let markdown = super::report::generate_markdown(&config, &violations, &[]);
    assert!(!json.contains("audit-secret-token"));
    assert!(!markdown.contains("audit-secret-token"));
    assert!(json.contains("[REDACTED]"));
    assert!(markdown.contains("[REDACTED]"));
}

#[test]
#[cfg(unix)]
fn audit_skips_file_symlinks() {
    use std::os::unix::fs::symlink;

    let test_dir = TestDir::new("symlink-boundary");
    let outside = TestDir::new("symlink-outside");
    let source = test_dir.child("src");
    let outside_file = outside.0.root.join("outside.rs");
    fs::write(&outside_file, "forbidden").expect("write outside fixture");
    symlink(outside_file, source.join("linked.rs")).expect("create symlink");
    let rule = HardRule {
        name: "boundary".into(),
        severity: "blocker".into(),
        paths: Vec::new(),
        extensions: vec!["rs".into()],
        patterns: vec!["forbidden".into()],
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    };

    let violations = scan_files(
        &test_dir.0,
        &[source],
        &[],
        &rule,
        &configured_audit().engine,
    )
    .expect("scan symlink fixture");
    assert!(violations.is_empty());
}

#[test]
fn audit_rejects_an_oversized_source_before_reading_it() {
    let test_dir = TestDir::new("oversized-source");
    let source = test_dir.child("src");
    fs::File::create(source.join("large.rs"))
        .expect("create large source")
        .set_len(16 * 1024 * 1024 + 1)
        .expect("size large source");
    let rule = HardRule {
        name: "bounded source".into(),
        severity: "blocker".into(),
        paths: Vec::new(),
        extensions: vec!["rs".into()],
        patterns: vec!["forbidden".into()],
        exclude_patterns: Vec::new(),
        allowlist: Vec::new(),
    };

    let error = scan_files(
        &test_dir.0,
        &[source],
        &[],
        &rule,
        &configured_audit().engine,
    )
    .expect_err("oversized source must fail closed");

    assert!(error.to_string().contains("is too large"));
}

#[test]
fn log_parser_keeps_the_error_trace() {
    let test_dir = TestDir::new("parse-logs");
    let input = test_dir.0.join("input.jsonl");
    let output = test_dir.0.join("output.json");
    fs::write(
        &input,
        concat!(
            "{\"level\":\"INFO\",\"trace_id\":\"failed\",\"fields\":{\"message\":\"start\"}}\n",
            "{\"level\":\"ERROR\",\"trace_id\":\"failed\",\"fields\":{\"error\":\"root cause\"}}\n",
            "{\"level\":\"INFO\",\"trace_id\":\"other\",\"fields\":{\"message\":\"later\"}}\n"
        ),
    )
    .expect("write log fixture");

    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
        .expect("parse logs");
    let parsed: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(output).expect("read output")).expect("output JSON");

    assert_eq!(parsed.len(), 2);
    assert!(parsed.iter().all(|entry| entry["trace_id"] == "failed"));
}

#[test]
fn log_parser_reads_trace_id_from_the_current_span() {
    let test_dir = TestDir::new("parse-span-logs");
    let input = test_dir.0.join("input.jsonl");
    let output = test_dir.0.join("output.json");
    fs::write(
            &input,
            concat!(
                "{\"level\":\"INFO\",\"span\":{\"trace_id\":\"span-trace\"},\"fields\":{\"message\":\"start\"}}\n",
                "{\"level\":\"ERROR\",\"span\":{\"trace_id\":\"span-trace\"},\"fields\":{\"error\":\"root cause\"}}\n"
            ),
        )
        .expect("write log fixture");

    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
        .expect("parse logs");
    let parsed: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(output).expect("read output")).expect("output JSON");

    assert_eq!(parsed.len(), 2);
    assert!(parsed.iter().all(|entry| entry["trace_id"] == "span-trace"));
}

#[test]
fn log_parser_keeps_the_error_in_a_long_trace() {
    let test_dir = TestDir::new("parse-long-logs");
    let input = test_dir.0.join("input.jsonl");
    let output = test_dir.0.join("output.json");
    let mut logs = String::new();
    for index in 0..35 {
        logs.push_str(&format!(
                "{{\"level\":\"INFO\",\"trace_id\":\"long-trace\",\"fields\":{{\"message\":\"before {index}\"}}}}\n"
            ));
    }
    logs.push_str(
            "{\"level\":\"ERROR\",\"trace_id\":\"long-trace\",\"fields\":{\"error\":\"retained root cause\"}}\n",
        );
    for index in 0..10 {
        logs.push_str(&format!(
                "{{\"level\":\"INFO\",\"trace_id\":\"long-trace\",\"fields\":{{\"message\":\"after {index}\"}}}}\n"
            ));
    }
    fs::write(&input, logs).expect("write log fixture");

    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
        .expect("parse logs");
    let parsed: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(output).expect("read output")).expect("output JSON");

    assert_eq!(parsed.len(), 30);
    assert!(parsed
        .iter()
        .any(|entry| entry["error"] == "retained root cause"));
}

#[test]
fn log_parser_falls_back_to_the_last_thirty_raw_lines() {
    let test_dir = TestDir::new("parse-unstructured-logs");
    let input = test_dir.0.join("input.log");
    let output = test_dir.0.join("output.log");
    let logs = (0..40)
        .map(|index| format!("unstructured line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&input, logs).expect("write unstructured log fixture");

    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
        .expect("parse unstructured logs");
    let extracted = fs::read_to_string(output).expect("read fallback output");
    let lines = extracted.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 30);
    assert_eq!(lines.first(), Some(&"unstructured line 10"));
    assert_eq!(lines.last(), Some(&"unstructured line 39"));
}

#[test]
fn log_parser_redacts_structured_and_fallback_output() {
    let test_dir = TestDir::new("parse-redacted-logs");
    let input = test_dir.0.join("input.jsonl");
    let output = test_dir.0.join("output.json");
    fs::write(
        &input,
        "{\"level\":\"ERROR\",\"trace_id\":\"secret-trace\",\"fields\":{\"error\":\"Authorization: Bearer log-secret-token\"}}\n",
    )
    .expect("write structured log fixture");
    log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
        .expect("parse structured logs");
    let structured = fs::read_to_string(&output).expect("read structured output");
    assert!(!structured.contains("log-secret-token"));

    let fallback_input = test_dir.0.join("fallback.log");
    let fallback_output = test_dir.0.join("fallback.out");
    fs::write(
        &fallback_input,
        "Authorization: Bearer fallback-secret-token\nplain line",
    )
    .expect("write fallback fixture");
    log_parser::extract_error_context(
        &fallback_input.to_string_lossy(),
        &fallback_output.to_string_lossy(),
    )
    .expect("parse fallback logs");
    let fallback = fs::read_to_string(fallback_output).expect("read fallback output");
    assert!(!fallback.contains("fallback-secret-token"));
}

#[test]
fn log_parser_rejects_an_oversized_line_before_retaining_it() {
    let test_dir = TestDir::new("parse-oversized-line");
    let input = test_dir.0.join("input.log");
    let output = test_dir.0.join("output.log");
    let line = "x".repeat(1024 * 1024 + 1);
    fs::write(&input, line).expect("write oversized log fixture");
    let error =
        log_parser::extract_error_context(&input.to_string_lossy(), &output.to_string_lossy())
            .expect_err("oversized log line must fail closed");
    assert!(error.to_string().contains("exceeds 1048576 bytes"));
}
