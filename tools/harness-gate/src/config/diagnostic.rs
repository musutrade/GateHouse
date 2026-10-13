use crate::failure::RetryClass;
use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;
const MAX_DIAGNOSTICS: usize = 50;

/// A deliberately small, valid starting point for users repairing a missing
/// or malformed workflow configuration. The generated preset also creates the
/// required audit and secret-scan files; this snippet is only the flow shape.
pub const MINIMAL_CONFIG_SNIPPET: &str = r#"version = 2

[project]
name = "my-project"
default_profile = "full"
hook_profile = "hook"

[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[scope]
unmatched = "all"
rules = [{ patterns = ["**"], components = ["project"] }]

[[steps]]
id = "project.diff-check"
label = "Git whitespace check"
component = "project"
profiles = ["full"]
program = "git"
args = ["diff", "--check"]
cwd = "{root}"
log = "git_diff_check.log"
timeout_secs = 60
"#;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RelatedDiagnostic {
    pub path: String,
    pub relation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub id: String,
    pub severity: DiagnosticSeverity,
    pub path: String,
    pub message: String,
    pub help: String,
    /// Configuration diagnostics are normally not retryable. The optional
    /// typed field lets future diagnostics express a retry class without
    /// making consumers parse human wording.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_class: Option<RetryClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigCheckReport {
    pub schema_version: u32,
    pub valid: bool,
    pub truncated: bool,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct ConfigDiagnostics {
    source: Option<PathBuf>,
    truncated: bool,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl ConfigDiagnostics {
    pub fn empty() -> Self {
        Self {
            source: None,
            truncated: false,
            diagnostics: Vec::new(),
        }
    }

    pub fn single(
        id: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        let mut diagnostics = Self::empty();
        diagnostics.push(ConfigDiagnostic {
            id: id.into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: message.into(),
            help: help.into(),
            retry_class: None,
            location: None,
            related: Vec::new(),
        });
        diagnostics
    }

    pub fn with_source(mut self, source: impl Into<PathBuf>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn push(&mut self, diagnostic: ConfigDiagnostic) {
        if self.truncated {
            return;
        }
        if self.diagnostics.len() < MAX_DIAGNOSTICS {
            self.diagnostics.push(diagnostic);
            return;
        }
        // Keep the published maximum strict: replace the last collected item
        // with the deterministic truncation notice once a further error proves
        // that the full set cannot be represented in this response.
        self.diagnostics.pop();
        self.truncated = true;
        self.diagnostics.push(ConfigDiagnostic {
            id: "HGCFG-DIAGNOSTICS-TRUNCATED".into(),
            severity: DiagnosticSeverity::Error,
            path: "$".into(),
            message: format!("configuration has more than {MAX_DIAGNOSTICS} independent errors"),
            help: "fix the reported errors and run `harness-gate config check` again".into(),
            retry_class: None,
            location: None,
            related: Vec::new(),
        });
    }

    pub fn report(&self) -> ConfigCheckReport {
        ConfigCheckReport {
            schema_version: DIAGNOSTIC_SCHEMA_VERSION,
            valid: self.diagnostics.is_empty(),
            truncated: self.truncated,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn sort(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            let left_key = left
                .location
                .as_ref()
                .map(|location| (0, location.line, location.column))
                .unwrap_or((1, 0, 0));
            let right_key = right
                .location
                .as_ref()
                .map(|location| (0, location.line, location.column))
                .unwrap_or((1, 0, 0));
            left_key
                .cmp(&right_key)
                .then_with(|| left.path.cmp(&right.path))
                .then_with(|| left.id.cmp(&right.id))
        });
        for diagnostic in &mut self.diagnostics {
            diagnostic.related.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then_with(|| left.relation.cmp(&right.relation))
            });
        }
    }

    pub fn render_human(&self) -> String {
        self.diagnostics
            .iter()
            .map(|diagnostic| {
                let mut rendered = String::new();
                if let Some(source) = &self.source {
                    rendered.push_str(&format!("{}: ", source.display()));
                }
                rendered.push_str(&format!(
                    "{} [{}] at {}: {}",
                    match diagnostic.severity {
                        DiagnosticSeverity::Error => "error",
                    },
                    diagnostic.id,
                    diagnostic.path,
                    diagnostic.message
                ));
                if let Some(location) = &diagnostic.location {
                    rendered.push_str(&format!(
                        " (line {}, column {})",
                        location.line, location.column
                    ));
                }
                for related in &diagnostic.related {
                    rendered.push_str(&format!("\n  {}: {}", related.relation, related.path));
                    if let Some(location) = &related.location {
                        rendered.push_str(&format!(
                            " (line {}, column {})",
                            location.line, location.column
                        ));
                    }
                }
                rendered.push_str(&format!("\n  help: {}", diagnostic.help));
                rendered
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn report_for_error(error: &anyhow::Error) -> ConfigCheckReport {
    if let Some(diagnostics) = error.downcast_ref::<ConfigDiagnostics>() {
        return diagnostics.report();
    }
    // Unknown errors intentionally receive one bounded generic report. The
    // machine contract must never classify an error by searching its prose.
    ConfigDiagnostics::single(
        "HGCFG-CHECK",
        "$",
        "configuration could not be checked",
        "verify the project root, configuration path, and required security configuration files",
    )
    .report()
}

impl fmt::Display for ConfigDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.render_human())
    }
}

impl Error for ConfigDiagnostics {}

#[derive(Debug, Clone)]
pub(super) struct SourceMap {
    locations: BTreeMap<String, SourceLocation>,
}

impl SourceMap {
    pub(super) fn from_source(source: &str) -> Self {
        let mut locations = BTreeMap::new();
        let mut table = String::new();
        let mut step_index = 0usize;

        for (line_index, line) in source.lines().enumerate() {
            let uncommented = strip_toml_comment(line);
            let trimmed = uncommented.trim_start();
            let column = line.len() - trimmed.len() + 1;
            if let Some(name) = trimmed
                .strip_prefix("[[")
                .and_then(|value| value.strip_suffix("]]"))
            {
                if name == "steps" {
                    table = format!("steps[{step_index}]");
                    locations.insert(
                        table.clone(),
                        SourceLocation {
                            line: line_index + 1,
                            column,
                        },
                    );
                    step_index += 1;
                } else {
                    table = canonical_table_path(name);
                    locations.insert(
                        table.clone(),
                        SourceLocation {
                            line: line_index + 1,
                            column,
                        },
                    );
                }
                continue;
            }
            if let Some(name) = trimmed
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
            {
                table = canonical_table_path(name);
                locations.insert(
                    table.clone(),
                    SourceLocation {
                        line: line_index + 1,
                        column,
                    },
                );
                continue;
            }
            let Some((key, _)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim().trim_matches(['"', '\'']);
            if key.is_empty() || key.starts_with('#') {
                continue;
            }
            let path = if table.is_empty() {
                key.to_string()
            } else {
                format!("{table}.{key}")
            };
            locations.insert(
                path,
                SourceLocation {
                    line: line_index + 1,
                    column,
                },
            );
        }
        Self { locations }
    }

    pub(super) fn location(&self, path: &str) -> Option<SourceLocation> {
        let mut candidate = path.to_string();
        loop {
            if let Some(location) = self.locations.get(&candidate) {
                return Some(location.clone());
            }
            if let Some(index) = candidate.rfind('[') {
                let suffix = &candidate[index + 1..];
                if suffix.starts_with('"')
                    || suffix
                        .chars()
                        .next()
                        .is_some_and(|value| value.is_ascii_digit())
                {
                    candidate.truncate(index);
                    continue;
                }
            }
            if let Some(index) = candidate.rfind('.') {
                candidate.truncate(index);
                continue;
            }
            return None;
        }
    }
}

pub(super) fn location_for_offset(source: &str, offset: usize) -> SourceLocation {
    let clamped = offset.min(source.len());
    let before = &source[..clamped];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map_or(before.chars().count() + 1, |(_, current_line)| {
            current_line.chars().count() + 1
        });
    SourceLocation { line, column }
}

pub(super) fn parse_diagnostic(
    source: &str,
    error: toml::de::Error,
    source_path: Option<&Path>,
) -> ConfigDiagnostics {
    let location = error
        .span()
        .map(|span| location_for_offset(source, span.start));
    let diagnostic = ConfigDiagnostic {
        id: "HGCFG-TOML-PARSE".into(),
        severity: DiagnosticSeverity::Error,
        path: "$".into(),
        message: "configuration is not valid TOML".into(),
        help: "fix the TOML syntax or unknown field reported by the parser".into(),
        retry_class: None,
        location,
        related: Vec::new(),
    };
    let mut diagnostics = ConfigDiagnostics::empty();
    if let Some(path) = source_path {
        diagnostics = diagnostics.with_source(path);
    }
    diagnostics.push(diagnostic);
    diagnostics
}

pub(super) fn interpolation_diagnostic(
    source: &str,
    range: Range<usize>,
    message: impl Into<String>,
    help: impl Into<String>,
    source_path: Option<&Path>,
) -> ConfigDiagnostics {
    let mut diagnostics = ConfigDiagnostics::empty();
    if let Some(path) = source_path {
        diagnostics = diagnostics.with_source(path);
    }
    diagnostics.push(ConfigDiagnostic {
        id: "HGCFG-INTERPOLATION".into(),
        severity: DiagnosticSeverity::Error,
        path: "$".into(),
        message: message.into(),
        help: help.into(),
        retry_class: None,
        location: Some(location_for_offset(source, range.start)),
        related: Vec::new(),
    });
    diagnostics
}

pub(super) fn audit_config_interpolation_diagnostic(
    source: &str,
    source_map: &SourceMap,
    source_path: Option<&Path>,
) -> ConfigDiagnostics {
    let mut diagnostics = ConfigDiagnostics::single(
        "HGCFG-PROJECT-SCOPED-CONFIG",
        "paths.audit_config",
        "audit configuration paths may not use environment interpolation",
        "set a repository-relative audit configuration path in flow.toml; use --project-root or --config to select a project",
    );
    if let Some(path) = source_path {
        diagnostics = diagnostics.with_source(path);
    }
    if let Some(diagnostic) = diagnostics.diagnostics.first_mut() {
        diagnostic.location = source_map
            .location("paths.audit_config")
            .or_else(|| Some(location_for_offset(source, 0)));
    }
    diagnostics
}

fn canonical_table_path(table: &str) -> String {
    let parts = table.split('.').collect::<Vec<_>>();
    match parts.as_slice() {
        ["services", id] => format!("services[\"{}\"]", id.trim_matches(['"', '\''])),
        ["parsers", id] => format!("parsers[\"{}\"]", id.trim_matches(['"', '\''])),
        ["paths", "aliases", id] => format!("paths.aliases[\"{}\"]", id.trim_matches(['"', '\''])),
        _ => table.into(),
    }
}

fn strip_toml_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in line.char_indices() {
        if let Some(delimiter) = quote {
            if delimiter == '"' && escaped {
                escaped = false;
            } else if delimiter == '"' && character == '\\' {
                escaped = true;
            } else if character == delimiter {
                quote = None;
            }
        } else if matches!(character, '"' | '\'') {
            quote = Some(character);
        } else if character == '#' {
            return &line[..offset];
        }
    }
    line
}
