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
profiles = ["full", "hook"]
program = "git"
args = ["diff", "--check"]
cwd = "{root}"
log = "git_diff_check.log"
timeout_secs = 60
"#;

/// Closed registry of configuration and plugin diagnostic codes. `CONFIG_CODES`
/// is the single mapping from a variant to its wire string and explanation; the
/// required documentation check verifies that this table covers every variant
/// exactly once and that each code has a heading, body and detailed guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigCode {
    HgcfgVersion,
    HgcfgRunnerVersion,
    HgcfgDependencyCycle,
    HgcfgDependency,
    HgcfgServiceInjectCollision,
    HgcfgUnknownReference,
    HgcfgInvalidPath,
    HgcfgInvalidEnvironment,
    HgcfgDuplicateField,
    HgcfgInvalidLog,
    HgcfgInvalidField,
    HgcfgRead,
    HgcfgExternalServiceMigration,
    HgcfgEnvironmentOverride,
    HgcfgDiagnosticsTruncated,
    HgcfgCheck,
    HgcfgTomlParse,
    HgcfgInterpolation,
    HgcfgProjectScopedConfig,
    HgcfgQuality,
    HgcfgRequiredFile,
    HgcfgImpact,
    HgcfgDuplicateLog,
    HgcfgSharedService,
    HgcfgTemplatePath,
    HgpluginCatalog,
    HgpluginCore,
    HgpluginDependency,
    HgpluginDigest,
    HgpluginLimit,
    HgpluginLock,
    HgpluginManifest,
    HgpluginMissing,
    HgpluginPath,
    HgpluginPlatform,
    HgpluginProtocol,
    HgpluginSignature,
    HgpluginStore,
    HgpluginVersion,
}

struct ConfigCodeEntry {
    code: ConfigCode,
    id: &'static str,
    title: &'static str,
    help: &'static str,
}

const CONFIG_CODES: &[ConfigCodeEntry] = &[
    ConfigCodeEntry {
        code: ConfigCode::HgcfgVersion,
        id: "HGCFG-VERSION",
        title: "workflow configuration version is unsupported",
        help: "migrate a v1 file with `harness-gate config migrate` or use version 2",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgRunnerVersion,
        id: "HGCFG-RUNNER-VERSION",
        title: "runner contract version is unsupported",
        help: "use the supported runner contract version",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgDependencyCycle,
        id: "HGCFG-DEPENDENCY-CYCLE",
        title: "dependencies contain a cycle",
        help: "remove the invalid dependency or add an explicit acyclic prerequisite",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgDependency,
        id: "HGCFG-DEPENDENCY",
        title: "dependency declaration is invalid",
        help: "remove the invalid dependency or add an explicit acyclic prerequisite",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgServiceInjectCollision,
        id: "HGCFG-SERVICE-INJECT-COLLISION",
        title: "service injection variable conflicts with another service or runner setting",
        help: "Use distinct service injection and runner thread variable names. For conflicts across unordered steps, add the intended dependency, use distinct names or split the workflow.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgUnknownReference,
        id: "HGCFG-UNKNOWN-REFERENCE",
        title: "reference does not name a declared configuration entry",
        help: "use an identifier declared in the configuration",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgInvalidPath,
        id: "HGCFG-INVALID-PATH",
        title: "path is not a safe repository-relative path",
        help: "use a non-empty path below the repository without a prefix or parent traversal",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgInvalidEnvironment,
        id: "HGCFG-INVALID-ENVIRONMENT",
        title: "environment variable name is invalid",
        help: "use an uppercase environment-variable name containing only letters, digits, and underscores",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgDuplicateField,
        id: "HGCFG-DUPLICATE-FIELD",
        title: "configuration declares the same identifier more than once",
        help: "keep one declaration or give each declaration a unique identifier",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgInvalidLog,
        id: "HGCFG-INVALID-LOG",
        title: "log path is not a valid single .log filename",
        help: "choose a unique single .log filename",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgInvalidField,
        id: "HGCFG-INVALID-FIELD",
        title: "field violates a documented configuration constraint",
        help: "update the field to satisfy the documented configuration constraints",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgRead,
        id: "HGCFG-READ",
        title: "Workflow configuration could not be read",
        help: "Check that the configured file exists, is readable UTF-8 and is below the 4 MiB size limit.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgExternalServiceMigration,
        id: "HGCFG-EXTERNAL-SERVICE-MIGRATION",
        title: "Legacy external service policies require migration",
        help: "Use the GateProbes recipe migration to create a new configuration, retain the original for rollback, and run config check before verification.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgEnvironmentOverride,
        id: "HGCFG-ENVIRONMENT-OVERRIDE",
        title: "Environment override has an invalid value",
        help: "Set the override named by the diagnostic path to the required type, or unset it. Do not print secret environment values.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgDiagnosticsTruncated,
        id: "HGCFG-DIAGNOSTICS-TRUNCATED",
        title: "Configuration diagnostic limit reached",
        help: "Fix the reported errors and run harness-gate config check again to reveal remaining diagnostics.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgCheck,
        id: "HGCFG-CHECK",
        title: "Configuration could not be checked",
        help: "Verify the project root, configuration path and required security configuration files; inspect the original command error.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgTomlParse,
        id: "HGCFG-TOML-PARSE",
        title: "Configuration is not valid TOML",
        help: "Inspect the reported line and column for malformed TOML or unsupported fields. Correct the source and run config check again.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgInterpolation,
        id: "HGCFG-INTERPOLATION",
        title: "Configuration environment interpolation failed",
        help: "Inspect the reported location and inline help. Supply required environment inputs or correct the placeholder syntax without exposing secret values.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgProjectScopedConfig,
        id: "HGCFG-PROJECT-SCOPED-CONFIG",
        title: "Security configuration path must remain project-scoped",
        help: "Use a literal repository-relative audit configuration path. Do not redirect the security policy through environment interpolation.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgQuality,
        id: "HGCFG-QUALITY",
        title: "Quality configuration is invalid",
        help: "Correct quality.toml references, profile participation and policy/collector authority. Preserve required checks and thresholds.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgRequiredFile,
        id: "HGCFG-REQUIRED-FILE",
        title: "Required security configuration file is missing",
        help: "Create the audit or secret-scan configuration file named by the diagnostic path, or correct its configured path; do not bypass the required check.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgImpact,
        id: "HGCFG-IMPACT",
        title: "Component impact graph is invalid",
        help: "Review the complete version-1 component graph. Declare an empty dependency list for components without dependencies.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgDuplicateLog,
        id: "HGCFG-DUPLICATE-LOG",
        title: "Verification steps share a log filename",
        help: "Choose a unique .log filename for each verification step; use the related location to find the other owner.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgSharedService,
        id: "HGCFG-SHARED-SERVICE",
        title: "Shared service users have no execution ordering",
        help: "Add an explicit dependency between the affected steps or define separate service resources.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgcfgTemplatePath,
        id: "HGCFG-TEMPLATE-PATH",
        title: "Report template path cannot be safely resolved",
        help: "Inspect the diagnostic path and related locations. Keep readable templates within their configured root and separate from report output; correct missing files or escaping symlinks.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginCatalog,
        id: "HGPLUGIN-CATALOG",
        title: "Signed preset catalog is invalid or incomplete",
        help: "Include the catalog and all referenced flow, recipe and pack files in the signed inventory and repair its flow configuration.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginCore,
        id: "HGPLUGIN-CORE",
        title: "Plugin does not declare this exact Core version in its preset manifest",
        help: "Select a reviewed plugin release whose signed preset manifest core_versions lists this exact Core version, or explicitly upgrade Core. This checks only the preset manifest declaration; it does not validate adapter requests.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginDependency,
        id: "HGPLUGIN-DEPENDENCY",
        title: "Plugin dependency is missing, incompatible or invalid",
        help: "Explicitly lock and prepare the exact dependency versions and required programs. Offline preflight never installs or runs them.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginDigest,
        id: "HGPLUGIN-DIGEST",
        title: "Plugin manifest or signed artifact digest does not match its pinned value",
        help: "Restore the exact pinned manifest when the lock manifest_sha256 does not match, and restore each artifact exactly as declared in the signed manifest file inventory. Do not change the lock or the signed inventory to hide corruption; review and explicitly lock any upgrade.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginLimit,
        id: "HGPLUGIN-LIMIT",
        title: "Plugin input exceeds a size or inventory limit",
        help: "The signed file inventory is checked for 1..128 entries before copying starts, and each input is checked against the 1 MiB read limit as it is copied. A later failure can leave earlier files only in the private temporary snapshot; there is no all-files precheck or cross-file atomicity.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginLock,
        id: "HGPLUGIN-LOCK",
        title: "Plugin lock is invalid or the selection is ambiguous",
        help: "Use harness-plugin-lock/v1 with 1..64 unique exact pins; review migration before replacing the lock. Absence of a lock preserves legacy workflows.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginManifest,
        id: "HGPLUGIN-MANIFEST",
        title: "Signed plugin manifest is invalid",
        help: "Prepare a harness-plugin/v1 data-only preset manifest and review compatibility before locking it.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginMissing,
        id: "HGPLUGIN-MISSING",
        title: "Prepared plugin input is missing",
        help: "Explicitly prepare or install the exact locked package in the selected offline store; preflight never downloads.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginPath,
        id: "HGPLUGIN-PATH",
        title: "Plugin input or signed artifact path is unsafe",
        help: "Use confined relative paths without parent traversal and prepare regular files, not symlinks.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginPlatform,
        id: "HGPLUGIN-PLATFORM",
        title: "Plugin does not support this exact host platform",
        help: "Prepare a reviewed package supporting this exact operating system and architecture.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginProtocol,
        id: "HGPLUGIN-PROTOCOL",
        title: "Plugin protocol is unsupported",
        help: "Select a data-only harness-preset-catalog/v1 package. Adapter execution still requires its separately signed v2 requests.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginSignature,
        id: "HGPLUGIN-SIGNATURE",
        title: "Plugin signature or host-approved key input is invalid",
        help: "Supply an operator-approved --plugin-trusted-key and a detached Ed25519 signature over the exact manifest. Project pins never authorize a key; nothing is downloaded or approved automatically.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginStore,
        id: "HGPLUGIN-STORE",
        title: "Plugin store is missing or was not supplied",
        help: "Pass --plugin-store PATH pointing to explicitly prepared offline packages.",
    },
    ConfigCodeEntry {
        code: ConfigCode::HgpluginVersion,
        id: "HGPLUGIN-VERSION",
        title: "Locked plugin identity or exact version is invalid",
        help: "Use a safe name and an exact numeric major.minor.patch version; install the exact locked product and never silently upgrade.",
    },
];

impl ConfigCode {
    fn entry(self) -> &'static ConfigCodeEntry {
        CONFIG_CODES
            .iter()
            .find(|entry| entry.code == self)
            .expect("registered configuration code")
    }

    pub(crate) fn all() -> impl Iterator<Item = ConfigCode> {
        CONFIG_CODES.iter().map(|entry| entry.code)
    }

    pub(crate) fn id(self) -> &'static str {
        self.entry().id
    }

    pub(crate) fn explain(self) -> (&'static str, &'static str) {
        let entry = self.entry();
        (entry.title, entry.help)
    }

    #[cfg(test)]
    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::all().find(|code| code.id() == id)
    }
}

/// Wire identity of one configuration diagnostic. It can only hold a registered
/// [`ConfigCode`]; there is no string or synthetic representation in production
/// or test builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiagnosticId(ConfigCode);

impl DiagnosticId {
    pub(crate) fn as_str(&self) -> &str {
        self.0.id()
    }
}

impl From<ConfigCode> for DiagnosticId {
    fn from(code: ConfigCode) -> Self {
        Self(code)
    }
}

impl fmt::Display for DiagnosticId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for DiagnosticId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Serialize for DiagnosticId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

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
    pub(crate) id: DiagnosticId,
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

impl ConfigDiagnostic {
    // Mirrors the serialized diagnostic fields; callers pass every field explicitly.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        code: ConfigCode,
        severity: DiagnosticSeverity,
        path: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
        retry_class: Option<RetryClass>,
        location: Option<SourceLocation>,
        related: Vec<RelatedDiagnostic>,
    ) -> Self {
        Self {
            id: DiagnosticId::from(code),
            severity,
            path: path.into(),
            message: message.into(),
            help: help.into(),
            retry_class,
            location,
            related,
        }
    }

    pub(crate) fn id(&self) -> &str {
        self.id.as_str()
    }
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
        code: ConfigCode,
        path: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        let mut diagnostics = Self::empty();
        diagnostics.push(ConfigDiagnostic::new(
            code,
            DiagnosticSeverity::Error,
            path,
            message,
            help,
            None,
            None,
            Vec::new(),
        ));
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
        self.diagnostics.push(ConfigDiagnostic::new(
            ConfigCode::HgcfgDiagnosticsTruncated,
            DiagnosticSeverity::Error,
            "$",
            format!("configuration has more than {MAX_DIAGNOSTICS} independent errors"),
            "fix the reported errors and run `harness-gate config check` again",
            None,
            None,
            Vec::new(),
        ));
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
                .then_with(|| left.id().cmp(right.id()))
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
                    diagnostic.id(),
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

    /// Terminal presentation uses only diagnostic metadata, never config values.
    /// Keep Display and the JSON report independent of terminal color settings.
    pub fn render_terminal(&self) -> String {
        self.diagnostics
            .iter()
            .map(|diagnostic| render_terminal_diagnostic(diagnostic, self.source.as_deref()))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

pub(super) fn terminal_text(text: &str) -> String {
    crate::utils::redaction::redact_text(text)
        .chars()
        .take(1024)
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}

fn terminal_location(source: Option<&Path>, location: Option<&SourceLocation>) -> String {
    let source = source
        .map(|path| terminal_text(&path.to_string_lossy()))
        .unwrap_or_else(|| "<configuration>".into());
    match location {
        Some(location) => format!("{source}:{}:{}", location.line, location.column),
        None => source,
    }
}

fn render_terminal_diagnostic(diagnostic: &ConfigDiagnostic, source: Option<&Path>) -> String {
    let code = terminal_text(diagnostic.id());
    let mut rendered = format!(
        "{}: {}\n  --> {}\n   |\n   = field: {}",
        crate::ui::error(format!("error[{code}]")),
        terminal_text(&diagnostic.message),
        terminal_location(source, diagnostic.location.as_ref()),
        terminal_text(&diagnostic.path),
    );
    for related in &diagnostic.related {
        rendered.push_str(&format!(
            "\n   = {}: {} ({})",
            terminal_text(&related.relation),
            terminal_text(&related.path),
            terminal_location(source, related.location.as_ref()),
        ));
    }
    rendered.push_str(&format!(
        "\n   = help: {}\n   = explain: harness-gate explain {code}",
        terminal_text(&diagnostic.help),
    ));
    rendered
}

pub fn report_for_error(error: &anyhow::Error) -> ConfigCheckReport {
    if let Some(diagnostics) = error.downcast_ref::<ConfigDiagnostics>() {
        return diagnostics.report();
    }
    // Unknown errors intentionally receive one bounded generic report. The
    // machine contract must never classify an error by searching its prose.
    ConfigDiagnostics::single(
        ConfigCode::HgcfgCheck,
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
    let diagnostic = ConfigDiagnostic::new(
        ConfigCode::HgcfgTomlParse,
        DiagnosticSeverity::Error,
        "$",
        "configuration is not valid TOML",
        "fix the TOML syntax or unknown field reported by the parser",
        None,
        location,
        Vec::new(),
    );
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
    diagnostics.push(ConfigDiagnostic::new(
        ConfigCode::HgcfgInterpolation,
        DiagnosticSeverity::Error,
        "$",
        message,
        help,
        None,
        Some(location_for_offset(source, range.start)),
        Vec::new(),
    ));
    diagnostics
}

pub(super) fn audit_config_interpolation_diagnostic(
    source: &str,
    source_map: &SourceMap,
    source_path: Option<&Path>,
) -> ConfigDiagnostics {
    let mut diagnostics = ConfigDiagnostics::single(
        ConfigCode::HgcfgProjectScopedConfig,
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

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn config_codes_are_unique_and_round_trip() {
        let mut seen_codes = std::collections::BTreeSet::new();
        let mut seen_ids = std::collections::BTreeSet::new();
        for code in ConfigCode::all() {
            assert!(
                seen_codes.insert(format!("{code:?}")),
                "duplicate code: {code:?}"
            );
            let id = code.id();
            assert!(seen_ids.insert(id), "duplicate configuration code: {id}");
            assert_eq!(ConfigCode::from_id(id), Some(code), "round trip: {id}");
            let (title, help) = code.explain();
            assert!(!title.is_empty(), "missing title: {id}");
            assert!(!help.is_empty(), "missing help: {id}");
        }
        assert_eq!(ConfigCode::from_id("HGCFG-NOT-REAL"), None);
        assert_eq!(ConfigCode::from_id("HGPLUGIN-NOT-REAL"), None);
    }

    #[test]
    fn diagnostic_id_serializes_as_a_plain_string_without_reordering_fields() {
        let diagnostic = ConfigDiagnostic::new(
            ConfigCode::HgcfgCheck,
            DiagnosticSeverity::Error,
            "$",
            "message",
            "help",
            None,
            None,
            Vec::new(),
        );
        let text = serde_json::to_string(&diagnostic).unwrap();
        assert!(
            text.starts_with(r#"{"id":"HGCFG-CHECK","severity":"error","path":"$","message":"message","help":"help"}"#),
            "{text}"
        );
        assert_eq!(
            serde_json::to_value(&diagnostic).unwrap()["id"],
            serde_json::json!("HGCFG-CHECK")
        );
    }

    #[test]
    fn diagnostic_id_is_closed_over_registered_codes() {
        let diagnostic = ConfigDiagnostic::new(
            ConfigCode::HgcfgCheck,
            DiagnosticSeverity::Error,
            "$",
            "m",
            "h",
            None,
            None,
            Vec::new(),
        );
        assert_eq!(diagnostic.id(), "HGCFG-CHECK");
        assert_eq!(
            serde_json::to_value(&diagnostic).unwrap()["id"],
            serde_json::json!("HGCFG-CHECK")
        );
    }
}
