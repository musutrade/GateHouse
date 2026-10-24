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

/// Closed registry of configuration and plugin diagnostic codes. Each variant
/// carries its wire string and explanation in exhaustive matches, so a new
/// production code cannot exist without an explanation. `ALL` also feeds
/// discovery and the documentation test.
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

impl ConfigCode {
    pub(crate) const ALL: &'static [ConfigCode] = &[
        ConfigCode::HgcfgVersion,
        ConfigCode::HgcfgRunnerVersion,
        ConfigCode::HgcfgDependencyCycle,
        ConfigCode::HgcfgDependency,
        ConfigCode::HgcfgServiceInjectCollision,
        ConfigCode::HgcfgUnknownReference,
        ConfigCode::HgcfgInvalidPath,
        ConfigCode::HgcfgInvalidEnvironment,
        ConfigCode::HgcfgDuplicateField,
        ConfigCode::HgcfgInvalidLog,
        ConfigCode::HgcfgInvalidField,
        ConfigCode::HgcfgRead,
        ConfigCode::HgcfgExternalServiceMigration,
        ConfigCode::HgcfgEnvironmentOverride,
        ConfigCode::HgcfgDiagnosticsTruncated,
        ConfigCode::HgcfgCheck,
        ConfigCode::HgcfgTomlParse,
        ConfigCode::HgcfgInterpolation,
        ConfigCode::HgcfgProjectScopedConfig,
        ConfigCode::HgcfgQuality,
        ConfigCode::HgcfgRequiredFile,
        ConfigCode::HgcfgImpact,
        ConfigCode::HgcfgDuplicateLog,
        ConfigCode::HgcfgSharedService,
        ConfigCode::HgcfgTemplatePath,
        ConfigCode::HgpluginCatalog,
        ConfigCode::HgpluginCore,
        ConfigCode::HgpluginDependency,
        ConfigCode::HgpluginDigest,
        ConfigCode::HgpluginLimit,
        ConfigCode::HgpluginLock,
        ConfigCode::HgpluginManifest,
        ConfigCode::HgpluginMissing,
        ConfigCode::HgpluginPath,
        ConfigCode::HgpluginPlatform,
        ConfigCode::HgpluginProtocol,
        ConfigCode::HgpluginSignature,
        ConfigCode::HgpluginStore,
        ConfigCode::HgpluginVersion,
    ];

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::HgcfgVersion => "HGCFG-VERSION",
            Self::HgcfgRunnerVersion => "HGCFG-RUNNER-VERSION",
            Self::HgcfgDependencyCycle => "HGCFG-DEPENDENCY-CYCLE",
            Self::HgcfgDependency => "HGCFG-DEPENDENCY",
            Self::HgcfgServiceInjectCollision => "HGCFG-SERVICE-INJECT-COLLISION",
            Self::HgcfgUnknownReference => "HGCFG-UNKNOWN-REFERENCE",
            Self::HgcfgInvalidPath => "HGCFG-INVALID-PATH",
            Self::HgcfgInvalidEnvironment => "HGCFG-INVALID-ENVIRONMENT",
            Self::HgcfgDuplicateField => "HGCFG-DUPLICATE-FIELD",
            Self::HgcfgInvalidLog => "HGCFG-INVALID-LOG",
            Self::HgcfgInvalidField => "HGCFG-INVALID-FIELD",
            Self::HgcfgRead => "HGCFG-READ",
            Self::HgcfgExternalServiceMigration => "HGCFG-EXTERNAL-SERVICE-MIGRATION",
            Self::HgcfgEnvironmentOverride => "HGCFG-ENVIRONMENT-OVERRIDE",
            Self::HgcfgDiagnosticsTruncated => "HGCFG-DIAGNOSTICS-TRUNCATED",
            Self::HgcfgCheck => "HGCFG-CHECK",
            Self::HgcfgTomlParse => "HGCFG-TOML-PARSE",
            Self::HgcfgInterpolation => "HGCFG-INTERPOLATION",
            Self::HgcfgProjectScopedConfig => "HGCFG-PROJECT-SCOPED-CONFIG",
            Self::HgcfgQuality => "HGCFG-QUALITY",
            Self::HgcfgRequiredFile => "HGCFG-REQUIRED-FILE",
            Self::HgcfgImpact => "HGCFG-IMPACT",
            Self::HgcfgDuplicateLog => "HGCFG-DUPLICATE-LOG",
            Self::HgcfgSharedService => "HGCFG-SHARED-SERVICE",
            Self::HgcfgTemplatePath => "HGCFG-TEMPLATE-PATH",
            Self::HgpluginCatalog => "HGPLUGIN-CATALOG",
            Self::HgpluginCore => "HGPLUGIN-CORE",
            Self::HgpluginDependency => "HGPLUGIN-DEPENDENCY",
            Self::HgpluginDigest => "HGPLUGIN-DIGEST",
            Self::HgpluginLimit => "HGPLUGIN-LIMIT",
            Self::HgpluginLock => "HGPLUGIN-LOCK",
            Self::HgpluginManifest => "HGPLUGIN-MANIFEST",
            Self::HgpluginMissing => "HGPLUGIN-MISSING",
            Self::HgpluginPath => "HGPLUGIN-PATH",
            Self::HgpluginPlatform => "HGPLUGIN-PLATFORM",
            Self::HgpluginProtocol => "HGPLUGIN-PROTOCOL",
            Self::HgpluginSignature => "HGPLUGIN-SIGNATURE",
            Self::HgpluginStore => "HGPLUGIN-STORE",
            Self::HgpluginVersion => "HGPLUGIN-VERSION",
        }
    }

    pub(crate) const fn explain(self) -> (&'static str, &'static str) {
        match self {
            Self::HgcfgVersion => (
                "workflow configuration version is unsupported",
                "migrate a v1 file with `harness-gate config migrate` or use version 2",
            ),
            Self::HgcfgRunnerVersion => (
                "runner contract version is unsupported",
                "use the supported runner contract version",
            ),
            Self::HgcfgDependencyCycle => (
                "dependencies contain a cycle",
                "remove the invalid dependency or add an explicit acyclic prerequisite",
            ),
            Self::HgcfgDependency => (
                "dependency declaration is invalid",
                "remove the invalid dependency or add an explicit acyclic prerequisite",
            ),
            Self::HgcfgServiceInjectCollision => (
                "service injection variable conflicts with another service or runner setting",
                "Use distinct service injection and runner thread variable names. For conflicts across unordered steps, add the intended dependency, use distinct names or split the workflow.",
            ),
            Self::HgcfgUnknownReference => (
                "reference does not name a declared configuration entry",
                "use an identifier declared in the configuration",
            ),
            Self::HgcfgInvalidPath => (
                "path is not a safe repository-relative path",
                "use a non-empty path below the repository without a prefix or parent traversal",
            ),
            Self::HgcfgInvalidEnvironment => (
                "environment variable name is invalid",
                "use an uppercase environment-variable name containing only letters, digits, and underscores",
            ),
            Self::HgcfgDuplicateField => (
                "configuration declares the same identifier more than once",
                "keep one declaration or give each declaration a unique identifier",
            ),
            Self::HgcfgInvalidLog => (
                "log path is not a valid single .log filename",
                "choose a unique single .log filename",
            ),
            Self::HgcfgInvalidField => (
                "field violates a documented configuration constraint",
                "update the field to satisfy the documented configuration constraints",
            ),
            Self::HgcfgRead => (
                "Workflow configuration could not be read",
                "Check that the configured file exists, is readable UTF-8 and is below the 4 MiB size limit.",
            ),
            Self::HgcfgExternalServiceMigration => (
                "Legacy external service policies require migration",
                "Use the GateProbes recipe migration to create a new configuration, retain the original for rollback, and run config check before verification.",
            ),
            Self::HgcfgEnvironmentOverride => (
                "Environment override has an invalid value",
                "Set the override named by the diagnostic path to the required type, or unset it. Do not print secret environment values.",
            ),
            Self::HgcfgDiagnosticsTruncated => (
                "Configuration diagnostic limit reached",
                "Fix the reported errors and run harness-gate config check again to reveal remaining diagnostics.",
            ),
            Self::HgcfgCheck => (
                "Configuration could not be checked",
                "Verify the project root, configuration path and required security configuration files; inspect the original command error.",
            ),
            Self::HgcfgTomlParse => (
                "Configuration is not valid TOML",
                "Inspect the reported line and column for malformed TOML or unsupported fields. Correct the source and run config check again.",
            ),
            Self::HgcfgInterpolation => (
                "Configuration environment interpolation failed",
                "Inspect the reported location and inline help. Supply required environment inputs or correct the placeholder syntax without exposing secret values.",
            ),
            Self::HgcfgProjectScopedConfig => (
                "Security configuration path must remain project-scoped",
                "Use a literal repository-relative audit configuration path. Do not redirect the security policy through environment interpolation.",
            ),
            Self::HgcfgQuality => (
                "Quality configuration is invalid",
                "Correct quality.toml references, profile participation and policy/collector authority. Preserve required checks and thresholds.",
            ),
            Self::HgcfgRequiredFile => (
                "Required security configuration file is missing",
                "Create the audit or secret-scan configuration file named by the diagnostic path, or correct its configured path; do not bypass the required check.",
            ),
            Self::HgcfgImpact => (
                "Component impact graph is invalid",
                "Review the complete version-1 component graph. Declare an empty dependency list for components without dependencies.",
            ),
            Self::HgcfgDuplicateLog => (
                "Verification steps share a log filename",
                "Choose a unique .log filename for each verification step; use the related location to find the other owner.",
            ),
            Self::HgcfgSharedService => (
                "Shared service users have no execution ordering",
                "Add an explicit dependency between the affected steps or define separate service resources.",
            ),
            Self::HgcfgTemplatePath => (
                "Report template path cannot be safely resolved",
                "Inspect the diagnostic path and related locations. Keep readable templates within their configured root and separate from report output; correct missing files or escaping symlinks.",
            ),
            Self::HgpluginCatalog => (
                "Signed preset catalog is invalid or incomplete",
                "Include the catalog and all referenced flow, recipe and pack files in the signed inventory and repair its flow configuration.",
            ),
            Self::HgpluginCore => (
                "Plugin does not declare this exact Core version in its preset manifest",
                "Select a reviewed plugin release whose signed preset manifest core_versions lists this exact Core version, or explicitly upgrade Core. This checks only the preset manifest declaration; it does not validate adapter requests.",
            ),
            Self::HgpluginDependency => (
                "Plugin dependency is missing, incompatible or invalid",
                "Explicitly lock and prepare the exact dependency versions and required programs. Offline preflight never installs or runs them.",
            ),
            Self::HgpluginDigest => (
                "Plugin manifest or signed artifact digest does not match its pinned value",
                "Restore the exact pinned manifest when the lock manifest_sha256 does not match, and restore each artifact exactly as declared in the signed manifest file inventory. Do not change the lock or the signed inventory to hide corruption; review and explicitly lock any upgrade.",
            ),
            Self::HgpluginLimit => (
                "Plugin input exceeds a size or inventory limit",
                "The signed file inventory is checked for 1..128 entries before copying starts, and each input is checked against the 1 MiB read limit as it is copied. A later failure can leave earlier files only in the private temporary snapshot; there is no all-files precheck or cross-file atomicity.",
            ),
            Self::HgpluginLock => (
                "Plugin lock is invalid or the selection is ambiguous",
                "Use harness-plugin-lock/v1 with 1..64 unique exact pins; review migration before replacing the lock. Absence of a lock preserves legacy workflows.",
            ),
            Self::HgpluginManifest => (
                "Signed plugin manifest is invalid",
                "Prepare a harness-plugin/v1 data-only preset manifest and review compatibility before locking it.",
            ),
            Self::HgpluginMissing => (
                "Prepared plugin input is missing",
                "Explicitly prepare or install the exact locked package in the selected offline store; preflight never downloads.",
            ),
            Self::HgpluginPath => (
                "Plugin input or signed artifact path is unsafe",
                "Use confined relative paths without parent traversal and prepare regular files, not symlinks.",
            ),
            Self::HgpluginPlatform => (
                "Plugin does not support this exact host platform",
                "Prepare a reviewed package supporting this exact operating system and architecture.",
            ),
            Self::HgpluginProtocol => (
                "Plugin protocol is unsupported",
                "Select a data-only harness-preset-catalog/v1 package. Adapter execution still requires its separately signed v2 requests.",
            ),
            Self::HgpluginSignature => (
                "Plugin signature or host-approved key input is invalid",
                "Supply an operator-approved --plugin-trusted-key and a detached Ed25519 signature over the exact manifest. Project pins never authorize a key; nothing is downloaded or approved automatically.",
            ),
            Self::HgpluginStore => (
                "Plugin store is missing or was not supplied",
                "Pass --plugin-store PATH pointing to explicitly prepared offline packages.",
            ),
            Self::HgpluginVersion => (
                "Locked plugin identity or exact version is invalid",
                "Use a safe name and an exact numeric major.minor.patch version; install the exact locked product and never silently upgrade.",
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|code| code.id() == id)
    }
}

/// Wire identity of one configuration diagnostic. Production code can only hold
/// a registered [`ConfigCode`]; tests may additionally use an explicit synthetic
/// value. Serialization is always the plain code string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticId(DiagnosticKind);

#[derive(Debug, Clone, PartialEq, Eq)]
enum DiagnosticKind {
    Registered(ConfigCode),
    // Constructed only by the cfg(test) synthetic constructor; production code
    // cannot name this private kind or its value.
    #[allow(dead_code)]
    Synthetic(String),
}

impl DiagnosticId {
    pub(crate) fn as_str(&self) -> &str {
        match &self.0 {
            DiagnosticKind::Registered(code) => code.id(),
            DiagnosticKind::Synthetic(value) => value,
        }
    }

    #[cfg(test)]
    pub(crate) fn synthetic(value: impl Into<String>) -> Self {
        Self(DiagnosticKind::Synthetic(value.into()))
    }
}

impl From<ConfigCode> for DiagnosticId {
    fn from(code: ConfigCode) -> Self {
        Self(DiagnosticKind::Registered(code))
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

    #[cfg(test)]
    pub(crate) fn synthetic(
        id: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        Self {
            id: DiagnosticId::synthetic(id),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: message.into(),
            help: help.into(),
            retry_class: None,
            location: None,
            related: Vec::new(),
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

    /// Test-only constructor for synthetic IDs that are not production codes.
    #[cfg(test)]
    pub(crate) fn synthetic(
        id: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        let mut diagnostics = Self::empty();
        diagnostics.push(ConfigDiagnostic::synthetic(id, path, message, help));
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
    fn config_codes_are_unique_explained_and_documented() {
        // The measured crate snapshot contains only tools/harness-gate, so the
        // repository-level documentation is read at runtime and checked when
        // present. Normal repository test runs always provide it.
        let documentation = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/failure-codes.md"),
        )
        .ok();
        let mut seen = std::collections::BTreeSet::new();
        for code in ConfigCode::ALL {
            let id = code.id();
            assert!(seen.insert(id), "duplicate configuration code: {id}");
            assert_eq!(ConfigCode::from_id(id), Some(*code), "round trip: {id}");
            let (title, help) = code.explain();
            assert!(!title.is_empty(), "missing title: {id}");
            assert!(!help.is_empty(), "missing help: {id}");
            if let Some(documentation) = documentation.as_deref() {
                let heading = format!("## {}", id.to_ascii_lowercase());
                assert!(
                    documentation.lines().any(|line| line == heading),
                    "missing independent documentation heading: {heading}"
                );
                let entry = format!("**{id} — ");
                assert!(
                    documentation.contains(&entry),
                    "missing documentation entry body: {entry}"
                );
            }
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
    fn synthetic_ids_remain_explicitly_test_only() {
        let synthetic = ConfigDiagnostic::synthetic("HGCFG-TEST-EXAMPLE", "$", "m", "h");
        assert_eq!(synthetic.id(), "HGCFG-TEST-EXAMPLE");
        assert_eq!(
            serde_json::to_value(&synthetic).unwrap()["id"],
            serde_json::json!("HGCFG-TEST-EXAMPLE")
        );
    }
}
