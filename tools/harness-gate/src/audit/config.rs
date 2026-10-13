use crate::project::resolve_repo_path;
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(super) const AUDIT_CONFIG_VERSION: u32 = 2;
pub(super) const AUDIT_MIGRATION_GUIDE: &str = "docs/configuration.md#audit-v2-migration";

#[derive(Debug, Default, Deserialize, Clone)]
pub(super) struct PathsConfig {
    #[serde(default)]
    pub(super) exclude: Vec<String>,
    /// 路径别名表，例如 backend = "backend"；规则里写别名即可
    #[serde(flatten)]
    pub(super) aliases: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub(super) version: u32,
    pub(super) engine: EngineConfig,
    #[serde(default)]
    pub(super) paths: PathsConfig,
    #[serde(default)]
    pub(super) hard_rules: Vec<HardRule>,
    #[serde(default)]
    pub(super) arch_rules: Vec<ArchRule>,
}

fn contains_legacy_string_allowlist(config: &toml::Value) -> bool {
    ["hard_rules", "arch_rules"].iter().any(|rules_key| {
        config
            .get(rules_key)
            .and_then(toml::Value::as_array)
            .is_some_and(|rules| {
                rules.iter().any(|rule| {
                    rule.get("allowlist")
                        .and_then(toml::Value::as_array)
                        .is_some_and(|entries| entries.iter().any(toml::Value::is_str))
                })
            })
    })
}

pub(super) fn parse_audit_config(source: &str) -> Result<Config> {
    let raw: toml::Value = toml::from_str(source).context("parse audit config TOML")?;
    let table = raw
        .as_table()
        .context("audit config must be a top-level TOML table")?;
    let version = match table.get("version") {
        Some(value) => value.as_integer().context(
            "audit config schema version must be an integer; see the audit v2 migration guide",
        )?,
        None => bail!(
            "audit config schema version is missing; migrate to schema v{AUDIT_CONFIG_VERSION}: \
             add `version = {AUDIT_CONFIG_VERSION}`, add `[engine]`, and convert string allowlist \
             entries to explicit `path-prefix` or `regex` entries; see {AUDIT_MIGRATION_GUIDE}"
        ),
    };
    if version != i64::from(AUDIT_CONFIG_VERSION) {
        bail!(
            "unsupported audit config schema version {version}; expected \
             {AUDIT_CONFIG_VERSION}; see {AUDIT_MIGRATION_GUIDE}"
        );
    }
    if !table.contains_key("engine") {
        bail!(
            "audit config schema v{AUDIT_CONFIG_VERSION} requires `[engine]`; copy the engine \
             defaults and comment syntax from the current preset; see {AUDIT_MIGRATION_GUIDE}"
        );
    }
    if contains_legacy_string_allowlist(&raw) {
        bail!(
            "audit config schema v{AUDIT_CONFIG_VERSION} no longer accepts string allowlist \
             entries; replace each string with `{{ kind = \"path-prefix\", path = \"...\" }}` or \
             `{{ kind = \"regex\", pattern = \"...\" }}`; see {AUDIT_MIGRATION_GUIDE}"
        );
    }

    toml::from_str(source).with_context(|| {
        format!("parse audit config schema v{AUDIT_CONFIG_VERSION}; see {AUDIT_MIGRATION_GUIDE}")
    })
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct EngineConfig {
    pub(super) ignore_filename: String,
    pub(super) json_report_filename: String,
    pub(super) markdown_report_filename: String,
    pub(super) markdown_max_bytes: usize,
    pub(super) markdown_occurrences_per_rule: usize,
    #[serde(default)]
    pub(super) comment_syntax: HashMap<String, CommentSyntax>,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct CommentSyntax {
    #[serde(default)]
    pub(super) line: Vec<String>,
    #[serde(default)]
    pub(super) block: Vec<BlockCommentSyntax>,
    #[serde(default)]
    pub(super) strings: Vec<StringSyntax>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockCommentSyntax {
    pub(super) start: String,
    pub(super) end: String,
    #[serde(default)]
    pub(super) nested: bool,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct StringSyntax {
    pub(super) start: String,
    pub(super) end: String,
    #[serde(default)]
    pub(super) escape: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum AllowlistEntry {
    PathPrefix { path: String },
    Regex { pattern: String },
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct HardRule {
    pub(super) name: String,
    pub(super) severity: String,
    pub(super) paths: Vec<String>,
    pub(super) extensions: Vec<String>,
    pub(super) patterns: Vec<String>,
    #[serde(default)]
    pub(super) exclude_patterns: Vec<String>,
    #[serde(default)]
    pub(super) allowlist: Vec<AllowlistEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ArchRule {
    pub(super) name: String,
    pub(super) layer: String,
    pub(super) paths: Vec<String>,
    pub(super) extensions: Vec<String>,
    pub(super) forbidden_patterns: Vec<String>,
    #[serde(default)]
    pub(super) allowed_patterns: Vec<String>,
    pub(super) suggestion: String,
    #[serde(default)]
    pub(super) exclude_patterns: Vec<String>,
    #[serde(default)]
    pub(super) allowlist: Vec<AllowlistEntry>,
}

fn validate_filename(value: &str, field: &str) -> Result<()> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.components().count() != 1 || path.file_name().is_none() {
        bail!("audit engine {field} must be a filename without directory components");
    }
    Ok(())
}

fn validate_engine_config(engine: &EngineConfig) -> Result<()> {
    validate_filename(&engine.ignore_filename, "ignore_filename")?;
    validate_filename(&engine.json_report_filename, "json_report_filename")?;
    validate_filename(&engine.markdown_report_filename, "markdown_report_filename")?;
    if engine.json_report_filename == engine.markdown_report_filename {
        bail!("audit engine report filenames must be distinct");
    }
    if engine.markdown_max_bytes == 0 || engine.markdown_occurrences_per_rule == 0 {
        bail!("audit engine markdown limits must be positive");
    }
    for (extension, syntax) in &engine.comment_syntax {
        if extension.trim().is_empty()
            || syntax.line.iter().any(|token| token.is_empty())
            || syntax
                .block
                .iter()
                .any(|block| block.start.is_empty() || block.end.is_empty())
            || syntax.strings.iter().any(|string| {
                string.start.is_empty()
                    || string.end.is_empty()
                    || string.escape.as_ref().is_some_and(String::is_empty)
            })
        {
            bail!("audit engine comment syntax for {extension:?} contains an empty token");
        }
        if syntax
            .block
            .iter()
            .any(|block| block.nested && block.start == block.end)
        {
            bail!(
                "audit engine nested comment syntax for {extension:?} requires distinct delimiters"
            );
        }
    }
    Ok(())
}

fn validate_rule_extensions(
    engine: &EngineConfig,
    extensions: &[String],
    rule_name: &str,
) -> Result<()> {
    for extension in extensions {
        if extension.trim().is_empty() {
            bail!("audit rule {rule_name:?} contains an empty extension");
        }
        if !engine.comment_syntax.contains_key(extension) {
            bail!(
                "audit rule {rule_name:?} uses extension {extension:?} without \
                 `[engine.comment_syntax.{extension}]`; define its comments and string delimiters \
                 before scanning"
            );
        }
    }
    Ok(())
}

pub(super) fn validate_audit_config(project_root: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    if config.version != AUDIT_CONFIG_VERSION {
        bail!(
            "unsupported audit config schema version {}; expected {}",
            config.version,
            AUDIT_CONFIG_VERSION
        );
    }
    validate_engine_config(&config.engine)?;
    for (alias, path) in &config.paths.aliases {
        resolve_repo_path(
            project_root,
            Path::new(path),
            &format!("audit path alias {alias:?}"),
            false,
        )?;
    }
    let exclude_dirs = super::scanner::resolve_excludes(project_root, &config.paths.exclude)?;
    let mut names = HashSet::new();

    for rule in &config.hard_rules {
        if rule.name.trim().is_empty() || !names.insert(rule.name.as_str()) {
            bail!(
                "audit rule names must be non-empty and unique: {:?}",
                rule.name
            );
        }
        if !matches!(rule.severity.as_str(), "blocker" | "error" | "warning") {
            bail!(
                "audit rule {:?} has unsupported severity {:?}",
                rule.name,
                rule.severity
            );
        }
        if rule.extensions.is_empty() || rule.patterns.is_empty() {
            bail!(
                "audit rule {:?} requires extensions and patterns",
                rule.name
            );
        }
        validate_rule_extensions(&config.engine, &rule.extensions, &rule.name)?;
        super::scanner::resolve_rule_roots(
            project_root,
            &rule.paths,
            &config.paths.aliases,
            &rule.name,
        )?;
        super::scanner::compile_regexes(&rule.patterns)?;
        super::scanner::compile_regexes(&rule.exclude_patterns)?;
        validate_allowlist(project_root, &rule.allowlist, &rule.name)?;
    }
    for rule in &config.arch_rules {
        if rule.name.trim().is_empty() || !names.insert(rule.name.as_str()) {
            bail!(
                "audit rule names must be non-empty and unique: {:?}",
                rule.name
            );
        }
        if rule.layer.trim().is_empty()
            || rule.suggestion.trim().is_empty()
            || rule.extensions.is_empty()
            || rule.forbidden_patterns.is_empty()
        {
            bail!(
                "architecture rule {:?} requires layer, suggestion, extensions, and forbidden_patterns",
                rule.name
            );
        }
        validate_rule_extensions(&config.engine, &rule.extensions, &rule.name)?;
        super::scanner::resolve_rule_roots(
            project_root,
            &rule.paths,
            &config.paths.aliases,
            &rule.name,
        )?;
        super::scanner::compile_regexes(&rule.forbidden_patterns)?;
        super::scanner::compile_regexes(&rule.allowed_patterns)?;
        super::scanner::compile_regexes(&rule.exclude_patterns)?;
        validate_allowlist(project_root, &rule.allowlist, &rule.name)?;
    }
    Ok(exclude_dirs)
}

fn validate_allowlist(
    project_root: &Path,
    allowlist: &[AllowlistEntry],
    rule_name: &str,
) -> Result<()> {
    for entry in allowlist {
        match entry {
            AllowlistEntry::PathPrefix { path } => {
                resolve_repo_path(
                    project_root,
                    Path::new(path),
                    &format!("audit rule {rule_name:?} allowlist path"),
                    false,
                )?;
            }
            AllowlistEntry::Regex { pattern } => {
                Regex::new(pattern).with_context(|| {
                    format!("audit rule {rule_name:?} has invalid allowlist regex {pattern:?}")
                })?;
            }
        }
    }
    Ok(())
}
