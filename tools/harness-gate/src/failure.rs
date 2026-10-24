mod terminal;
pub(crate) use terminal::{quality_blockers, render as render_terminal};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable machine-facing failure registry. Display text is deliberately kept
/// separate so human wording can evolve without changing retry or reporting
/// behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum FailureCode {
    WebhookDestinationDenied,
    WebhookRedirectDenied,
    LeaseOwnershipUncertain,
    ServiceSetupFailure,
    ServiceLeaseFailure,
    ResultParseFailure,
    ResultZero,
    ResultPartial,
    SchedulerFailure,
    SecretScanFailure,
    ArchitectureAuditFailure,
    QualityBlocked,
    StepExecutionFailure,
    StepSkipped,
    OutputLimitExceeded,
    ReaderDeadlineExceeded,
    StepCancelled,
    StepTimeout,
    StepFailed,
    EvidencePathEscape,
    EvidencePending,
    EvidenceFinalizationFailure,
    EvidencePublicationFailure,
    EvidenceDuplicatePath,
    EvidenceStepUnbound,
    EvidenceInvocationMismatch,
    EvidenceMissing,
    EvidenceUndeclaredFile,
    EvidenceSymlink,
    EvidenceInvalidType,
    EvidenceReadFailure,
    EvidenceInvalidMetadata,
}

impl fmt::Display for FailureCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wire_value = serde_json::to_value(self).map_err(|_| fmt::Error)?;
        formatter.write_str(wire_value.as_str().ok_or(fmt::Error)?)
    }
}

impl TryFrom<&str> for FailureCode {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(value))
            .map_err(|_| ())
    }
}

/// Human guidance is supplementary; it never changes failure/retry decisions.
#[derive(Serialize)]
pub(crate) struct Explanation {
    code: String,
    title: &'static str,
    help: &'static str,
    documentation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'static str>,
}

const EXPLANATIONS: &[(&str, &str, &str)] = &[
    ("WEBHOOK_DESTINATION_DENIED", "URL host or resolved address is outside the egress policy", "Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks."),
    ("WEBHOOK_REDIRECT_DENIED", "Webhook returned a redirect while redirects are disabled", "Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks."),
    ("LEASE_OWNERSHIP_UNCERTAIN", "Lease identity or renewal cannot be proven", "Inspect the recorded owner and runtime identity. Use cleanup --dry-run to inspect stale leases; retain uncertain resources rather than deleting by name."),
    ("SERVICE_SETUP_FAILURE", "Service-manager lock acquisition failed", "Inspect cancellation, a poisoned mutex or the bounded service-manager lock wait. Resolve the original lock error before retrying; do not bypass synchronization."),
    ("SERVICE_LEASE_FAILURE", "A service lease could not be acquired", "Inspect the service setup log and required tools or credentials. Confirm resource ownership before cleanup, then retry the original verification."),
    ("RESULT_PARSE_FAILURE", "Configured test result could not be parsed", "Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results."),
    ("RESULT_ZERO", "A parser found no results", "Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results."),
    ("RESULT_PARTIAL", "A parser found fewer results than required", "Inspect the declared result format, output path and parser settings. Ensure the producing test command completed and emitted the required results."),
    ("SCHEDULER_FAILURE", "The verification scheduler or cleanup failed", "Inspect the first scheduler or cleanup failure and its step logs. Resolve the underlying error before rerunning verification."),
    ("SECRET_SCAN_FAILURE", "Secret scan found a blocking finding", "Inspect the finding without printing the secret. Remove exposed credentials from tracked inputs and rotate compromised values before rerunning the scan."),
    ("QUALITY_BLOCKED", "Generic quality policy failed or trusted workflow inputs/evidence are invalid", "Inspect the normalized findings and evidence-validation errors. Repair missing or invalid evidence, improve tested coverage or simplify risky code; preserve required gates and thresholds."),
    ("ARCHITECTURE_AUDIT_FAILURE", "Architecture audit found a blocking violation", "Inspect the reported rule and source location. Correct the dependency or boundary violation, then rerun the audit."),
    ("STEP_EXECUTION_FAILURE", "A configured step could not execute", "Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass."),
    ("STEP_SKIPPED", "A step was not dispatched because a prerequisite failed", "Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass."),
    ("OUTPUT_LIMIT_EXCEEDED", "A bounded process reader exceeded its byte budget", "Reduce unnecessary command output or correct runaway logging. Inspect the explicit output budget; truncated output cannot establish a successful result."),
    ("READER_DEADLINE_EXCEEDED", "A process reader exceeded its completion deadline", "Check whether child processes still hold stdout or stderr open. Ensure descendants terminate and close inherited pipes before retrying."),
    ("STEP_CANCELLED", "The run was cancelled before completion", "Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass."),
    ("STEP_TIMEOUT", "A step exceeded its timeout", "Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass."),
    ("STEP_FAILED", "A step exited unsuccessfully without a more specific code", "Inspect the step log, command, dependencies and execution environment. Correct the first failure before rerunning; a skipped or cancelled step is not a pass."),
    ("EVIDENCE_PATH_ESCAPE", "An artifact or step log escapes the invocation root", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_PENDING", "Evidence publication has not reached a complete state", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_FINALIZATION_FAILURE", "Evidence finalization failed before the machine result could be published", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_PUBLICATION_FAILURE", "A machine or human report could not be published atomically", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_DUPLICATE_PATH", "Multiple artifact bindings claim the same path", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_STEP_UNBOUND", "An artifact is not bound to a declared verification step", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_INVOCATION_MISMATCH", "An artifact or manifest belongs to another invocation", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_MISSING", "Required evidence is absent or its path cannot be inspected", "Inspect the original filesystem error to distinguish absence from permission or I/O failures. Repair the producer or access problem and generate fresh evidence; do not fabricate replacement artifacts."),
    ("EVIDENCE_UNDECLARED_FILE", "A file was found in the evidence root without a declaration", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_SYMLINK", "A symlink was found where a regular evidence file was required", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_INVALID_TYPE", "An evidence entry has an unsupported type or shape", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_READ_FAILURE", "An evidence file could not be read safely", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_INVALID_METADATA", "Evidence metadata is malformed or inconsistent", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
];

const CLI_EXPLANATIONS: &[(&str, &str, &str)] = &[
    ("E1000", "Command invocation failed", "Read the accompanying command error. Check arguments, explicit input paths and required files; use --help for the failing command. No successful verification is implied."),
    ("E1101", "Architecture audit configuration failed", "Inspect the audit configuration read or TOML/schema error. Correct the selected input before rerunning audit; later semantic rule validation can report E1102."),
    ("E1102", "Architecture audit execution failed", "Inspect the audit error and scan inputs. Correct the execution or access problem before rerunning the required audit."),
    ("E1103", "Audit log parsing failed", "Inspect input and output I/O, UTF-8 encoding and the bounded-line error. Retain the original log; unrecognized JSON can be skipped or fall back to a raw tail."),
    ("E1201", "Secret scan configuration failed", "Correct the named secret-scan configuration error. Keep required scanning enabled and avoid printing secret values."),
    ("E1202", "Secret scan failed", "Inspect the scan error or finding, correct unreadable inputs, and remove or rotate exposed credentials as applicable. Do not bypass required scanning."),
    ("E1301", "Git scope detection failed", "Check the selected repository, Git availability and requested revision or staged snapshot. Retry with the intended scope after correcting the Git error."),
    ("E1302", "Scope configuration failed", "Inspect impact-input paths, readability and pinned SHA256 freshness. Review or regenerate the graph and its pins together; do not blindly rewrite hashes."),
    ("E1303", "Changed files do not match scope rules", "Review each unmatched path and assign it to the correct component with reviewed scope rules. Do not exclude changed files merely to pass verification."),
    ("E1304", "Scope report failed", "Check the report destination and permissions. Correct the write failure and regenerate the scope report."),
    ("E1401", "Verification profile or step is invalid", "Check the configured profile and step identifiers and whether the step belongs to the selected profile. Use verify --plan to inspect the corrected execution plan."),
    ("E1402", "Verification was cancelled", "Identify the cancellation source and confirm cleanup outcomes. Rerun the intended verification after cancellation is resolved; cancelled work is not successful evidence."),
    ("E1403", "Verification execution failed", "Inspect the first execution error and step logs. Correct the command, environment or prerequisite failure, then generate fresh verification evidence."),
    ("E1404", "Verification report failed", "Identify the failed reporting stage: local publication, mirroring or notification. Local reports may already be complete; inspect webhook delivery and egress errors as applicable."),
];

fn config_explanations() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    crate::config::ConfigCode::all().map(|code| {
        let (title, help) = code.explain();
        (code.id(), title, help)
    })
}

const DETAILED_EXPLANATIONS: &[(&str, &str)] = &[
    (
        "ARCHITECTURE_AUDIT_FAILURE",
        include_str!("../docs/errors/ARCHITECTURE_AUDIT_FAILURE.md"),
    ),
    ("E1000", include_str!("../docs/errors/E1000.md")),
    ("E1101", include_str!("../docs/errors/E1101.md")),
    ("E1102", include_str!("../docs/errors/E1102.md")),
    ("E1103", include_str!("../docs/errors/E1103.md")),
    ("E1201", include_str!("../docs/errors/E1201.md")),
    ("E1202", include_str!("../docs/errors/E1202.md")),
    ("E1301", include_str!("../docs/errors/E1301.md")),
    ("E1302", include_str!("../docs/errors/E1302.md")),
    ("E1303", include_str!("../docs/errors/E1303.md")),
    ("E1304", include_str!("../docs/errors/E1304.md")),
    ("E1401", include_str!("../docs/errors/E1401.md")),
    ("E1402", include_str!("../docs/errors/E1402.md")),
    ("E1403", include_str!("../docs/errors/E1403.md")),
    ("E1404", include_str!("../docs/errors/E1404.md")),
    (
        "EVIDENCE_DUPLICATE_PATH",
        include_str!("../docs/errors/EVIDENCE_DUPLICATE_PATH.md"),
    ),
    (
        "EVIDENCE_FINALIZATION_FAILURE",
        include_str!("../docs/errors/EVIDENCE_FINALIZATION_FAILURE.md"),
    ),
    (
        "EVIDENCE_INVALID_METADATA",
        include_str!("../docs/errors/EVIDENCE_INVALID_METADATA.md"),
    ),
    (
        "EVIDENCE_INVALID_TYPE",
        include_str!("../docs/errors/EVIDENCE_INVALID_TYPE.md"),
    ),
    (
        "EVIDENCE_INVOCATION_MISMATCH",
        include_str!("../docs/errors/EVIDENCE_INVOCATION_MISMATCH.md"),
    ),
    (
        "EVIDENCE_MISSING",
        include_str!("../docs/errors/EVIDENCE_MISSING.md"),
    ),
    (
        "EVIDENCE_PATH_ESCAPE",
        include_str!("../docs/errors/EVIDENCE_PATH_ESCAPE.md"),
    ),
    (
        "EVIDENCE_PENDING",
        include_str!("../docs/errors/EVIDENCE_PENDING.md"),
    ),
    (
        "EVIDENCE_PUBLICATION_FAILURE",
        include_str!("../docs/errors/EVIDENCE_PUBLICATION_FAILURE.md"),
    ),
    (
        "EVIDENCE_READ_FAILURE",
        include_str!("../docs/errors/EVIDENCE_READ_FAILURE.md"),
    ),
    (
        "EVIDENCE_STEP_UNBOUND",
        include_str!("../docs/errors/EVIDENCE_STEP_UNBOUND.md"),
    ),
    (
        "EVIDENCE_SYMLINK",
        include_str!("../docs/errors/EVIDENCE_SYMLINK.md"),
    ),
    (
        "EVIDENCE_UNDECLARED_FILE",
        include_str!("../docs/errors/EVIDENCE_UNDECLARED_FILE.md"),
    ),
    ("HGCFG-CHECK", include_str!("../docs/errors/HGCFG-CHECK.md")),
    (
        "HGCFG-DEPENDENCY-CYCLE",
        include_str!("../docs/errors/HGCFG-DEPENDENCY-CYCLE.md"),
    ),
    (
        "HGCFG-DEPENDENCY",
        include_str!("../docs/errors/HGCFG-DEPENDENCY.md"),
    ),
    (
        "HGCFG-DIAGNOSTICS-TRUNCATED",
        include_str!("../docs/errors/HGCFG-DIAGNOSTICS-TRUNCATED.md"),
    ),
    (
        "HGCFG-DUPLICATE-FIELD",
        include_str!("../docs/errors/HGCFG-DUPLICATE-FIELD.md"),
    ),
    (
        "HGCFG-DUPLICATE-LOG",
        include_str!("../docs/errors/HGCFG-DUPLICATE-LOG.md"),
    ),
    (
        "HGCFG-ENVIRONMENT-OVERRIDE",
        include_str!("../docs/errors/HGCFG-ENVIRONMENT-OVERRIDE.md"),
    ),
    (
        "HGCFG-EXTERNAL-SERVICE-MIGRATION",
        include_str!("../docs/errors/HGCFG-EXTERNAL-SERVICE-MIGRATION.md"),
    ),
    (
        "HGCFG-IMPACT",
        include_str!("../docs/errors/HGCFG-IMPACT.md"),
    ),
    (
        "HGCFG-INTERPOLATION",
        include_str!("../docs/errors/HGCFG-INTERPOLATION.md"),
    ),
    (
        "HGCFG-INVALID-ENVIRONMENT",
        include_str!("../docs/errors/HGCFG-INVALID-ENVIRONMENT.md"),
    ),
    (
        "HGCFG-INVALID-FIELD",
        include_str!("../docs/errors/HGCFG-INVALID-FIELD.md"),
    ),
    (
        "HGCFG-INVALID-LOG",
        include_str!("../docs/errors/HGCFG-INVALID-LOG.md"),
    ),
    (
        "HGCFG-INVALID-PATH",
        include_str!("../docs/errors/HGCFG-INVALID-PATH.md"),
    ),
    (
        "HGCFG-PROJECT-SCOPED-CONFIG",
        include_str!("../docs/errors/HGCFG-PROJECT-SCOPED-CONFIG.md"),
    ),
    (
        "HGCFG-QUALITY",
        include_str!("../docs/errors/HGCFG-QUALITY.md"),
    ),
    ("HGCFG-READ", include_str!("../docs/errors/HGCFG-READ.md")),
    (
        "HGCFG-REQUIRED-FILE",
        include_str!("../docs/errors/HGCFG-REQUIRED-FILE.md"),
    ),
    (
        "HGCFG-RUNNER-VERSION",
        include_str!("../docs/errors/HGCFG-RUNNER-VERSION.md"),
    ),
    (
        "HGCFG-SERVICE-INJECT-COLLISION",
        include_str!("../docs/errors/HGCFG-SERVICE-INJECT-COLLISION.md"),
    ),
    (
        "HGCFG-SHARED-SERVICE",
        include_str!("../docs/errors/HGCFG-SHARED-SERVICE.md"),
    ),
    (
        "HGCFG-TEMPLATE-PATH",
        include_str!("../docs/errors/HGCFG-TEMPLATE-PATH.md"),
    ),
    (
        "HGCFG-TOML-PARSE",
        include_str!("../docs/errors/HGCFG-TOML-PARSE.md"),
    ),
    (
        "HGCFG-UNKNOWN-REFERENCE",
        include_str!("../docs/errors/HGCFG-UNKNOWN-REFERENCE.md"),
    ),
    (
        "HGCFG-VERSION",
        include_str!("../docs/errors/HGCFG-VERSION.md"),
    ),
    (
        "LEASE_OWNERSHIP_UNCERTAIN",
        include_str!("../docs/errors/LEASE_OWNERSHIP_UNCERTAIN.md"),
    ),
    (
        "OUTPUT_LIMIT_EXCEEDED",
        include_str!("../docs/errors/OUTPUT_LIMIT_EXCEEDED.md"),
    ),
    (
        "QUALITY_BLOCKED",
        include_str!("../docs/errors/QUALITY_BLOCKED.md"),
    ),
    (
        "READER_DEADLINE_EXCEEDED",
        include_str!("../docs/errors/READER_DEADLINE_EXCEEDED.md"),
    ),
    (
        "RESULT_PARSE_FAILURE",
        include_str!("../docs/errors/RESULT_PARSE_FAILURE.md"),
    ),
    (
        "RESULT_PARTIAL",
        include_str!("../docs/errors/RESULT_PARTIAL.md"),
    ),
    ("RESULT_ZERO", include_str!("../docs/errors/RESULT_ZERO.md")),
    (
        "SCHEDULER_FAILURE",
        include_str!("../docs/errors/SCHEDULER_FAILURE.md"),
    ),
    (
        "SECRET_SCAN_FAILURE",
        include_str!("../docs/errors/SECRET_SCAN_FAILURE.md"),
    ),
    (
        "SERVICE_LEASE_FAILURE",
        include_str!("../docs/errors/SERVICE_LEASE_FAILURE.md"),
    ),
    (
        "SERVICE_SETUP_FAILURE",
        include_str!("../docs/errors/SERVICE_SETUP_FAILURE.md"),
    ),
    (
        "STEP_CANCELLED",
        include_str!("../docs/errors/STEP_CANCELLED.md"),
    ),
    (
        "STEP_EXECUTION_FAILURE",
        include_str!("../docs/errors/STEP_EXECUTION_FAILURE.md"),
    ),
    ("STEP_FAILED", include_str!("../docs/errors/STEP_FAILED.md")),
    (
        "STEP_SKIPPED",
        include_str!("../docs/errors/STEP_SKIPPED.md"),
    ),
    (
        "STEP_TIMEOUT",
        include_str!("../docs/errors/STEP_TIMEOUT.md"),
    ),
    (
        "WEBHOOK_DESTINATION_DENIED",
        include_str!("../docs/errors/WEBHOOK_DESTINATION_DENIED.md"),
    ),
    (
        "WEBHOOK_REDIRECT_DENIED",
        include_str!("../docs/errors/WEBHOOK_REDIRECT_DENIED.md"),
    ),
    (
        "HGPLUGIN-CATALOG",
        include_str!("../docs/errors/HGPLUGIN-CATALOG.md"),
    ),
    (
        "HGPLUGIN-CORE",
        include_str!("../docs/errors/HGPLUGIN-CORE.md"),
    ),
    (
        "HGPLUGIN-DEPENDENCY",
        include_str!("../docs/errors/HGPLUGIN-DEPENDENCY.md"),
    ),
    (
        "HGPLUGIN-DIGEST",
        include_str!("../docs/errors/HGPLUGIN-DIGEST.md"),
    ),
    (
        "HGPLUGIN-LIMIT",
        include_str!("../docs/errors/HGPLUGIN-LIMIT.md"),
    ),
    (
        "HGPLUGIN-LOCK",
        include_str!("../docs/errors/HGPLUGIN-LOCK.md"),
    ),
    (
        "HGPLUGIN-MANIFEST",
        include_str!("../docs/errors/HGPLUGIN-MANIFEST.md"),
    ),
    (
        "HGPLUGIN-MISSING",
        include_str!("../docs/errors/HGPLUGIN-MISSING.md"),
    ),
    (
        "HGPLUGIN-PATH",
        include_str!("../docs/errors/HGPLUGIN-PATH.md"),
    ),
    (
        "HGPLUGIN-PLATFORM",
        include_str!("../docs/errors/HGPLUGIN-PLATFORM.md"),
    ),
    (
        "HGPLUGIN-PROTOCOL",
        include_str!("../docs/errors/HGPLUGIN-PROTOCOL.md"),
    ),
    (
        "HGPLUGIN-SIGNATURE",
        include_str!("../docs/errors/HGPLUGIN-SIGNATURE.md"),
    ),
    (
        "HGPLUGIN-STORE",
        include_str!("../docs/errors/HGPLUGIN-STORE.md"),
    ),
    (
        "HGPLUGIN-VERSION",
        include_str!("../docs/errors/HGPLUGIN-VERSION.md"),
    ),
];

fn detailed_explanation(code: &str) -> Option<&'static str> {
    DETAILED_EXPLANATIONS
        .iter()
        .find(|entry| entry.0 == code)
        .map(|entry| entry.1)
}

pub(crate) fn explain(code: &str, json: bool) -> anyhow::Result<bool> {
    let (_, title, help) = EXPLANATIONS
        .iter()
        .copied()
        .chain(CLI_EXPLANATIONS.iter().copied())
        .chain(config_explanations())
        .find(|entry| entry.0 == code)
        .ok_or_else(|| anyhow::anyhow!("unknown machine failure code or CLI error code or configuration diagnostic code; see docs/failure-codes.md for supported codes"))?;
    let explanation = Explanation {
        code: code.to_owned(),
        details: detailed_explanation(code),
        title,
        help,
        documentation: format!(
            "https://github.com/musutrade/GateHouse/blob/main/docs/failure-codes.md#{}",
            code.to_ascii_lowercase().replace('_', "-")
        ),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&explanation)?);
    } else {
        println!(
            "{}: {}\n\nNext steps: {}\n\nSee: {}",
            explanation.code, explanation.title, explanation.help, explanation.documentation
        );
        if let Some(details) = explanation.details {
            println!("\n{details}");
        }
    }
    Ok(true)
}

/// Catalog discovery never loads project configuration or evaluates policy.
pub(crate) fn discover(search: Option<&str>, json: bool) -> anyhow::Result<bool> {
    let query = search.unwrap_or("").trim().to_ascii_lowercase();
    anyhow::ensure!(
        search.is_none() || !query.is_empty(),
        "search text must not be empty"
    );
    let mut entries: Vec<_> = EXPLANATIONS
        .iter()
        .copied()
        .chain(CLI_EXPLANATIONS.iter().copied())
        .chain(config_explanations())
        .filter(|(code, title, help)| {
            format!("{code} {title} {help}")
                .to_ascii_lowercase()
                .contains(&query)
        })
        .map(|(code, title, help)| Explanation {
            code: code.to_owned(),
            details: detailed_explanation(code),
            title,
            help,
            documentation: format!(
                "https://github.com/musutrade/GateHouse/blob/main/docs/failure-codes.md#{}",
                code.to_ascii_lowercase().replace('_', "-")
            ),
        })
        .collect();
    entries.sort_by(|left, right| left.code.cmp(&right.code));
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if entries.is_empty() {
        println!("No matching error codes.");
    } else {
        for entry in entries {
            println!("{}: {}", entry.code, entry.title);
        }
    }
    Ok(true)
}

/// Retry classes are a closed configuration vocabulary. Serde keeps the
/// existing lowercase TOML/JSON spelling while rejecting unknown classes.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RetryClass {
    Cancelled,
    Timeout,
    Parser,
    Exit,
}

impl fmt::Display for RetryClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cancelled => "cancelled",
            Self::Timeout => "timeout",
            Self::Parser => "parser",
            Self::Exit => "exit",
        })
    }
}

impl TryFrom<&str> for RetryClass {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "cancelled" => Ok(Self::Cancelled),
            "timeout" => Ok(Self::Timeout),
            "parser" => Ok(Self::Parser),
            "exit" => Ok(Self::Exit),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FailureCode, RetryClass};

    #[test]
    fn explanations_cover_the_closed_registry_without_duplicates() {
        let schema = serde_json::to_value(schemars::schema_for!(FailureCode)).unwrap();
        let codes = schema["enum"].as_array().unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for &(code, title, help) in super::EXPLANATIONS {
            assert!(FailureCode::try_from(code).is_ok());
            assert!(seen.insert(code), "duplicate explanation: {code}");
            assert!(!title.is_empty() && !help.is_empty());
        }
        assert_eq!(seen.len(), codes.len());
        for code in codes {
            assert!(seen.contains(code.as_str().unwrap()));
        }
        assert!(super::explain("UNKNOWN_CODE", false).is_err());
    }

    #[test]
    fn failure_code_wire_names_are_stable_and_closed() {
        let expected = [
            "ARCHITECTURE_AUDIT_FAILURE",
            "EVIDENCE_DUPLICATE_PATH",
            "EVIDENCE_FINALIZATION_FAILURE",
            "EVIDENCE_INVALID_METADATA",
            "EVIDENCE_INVALID_TYPE",
            "EVIDENCE_INVOCATION_MISMATCH",
            "EVIDENCE_MISSING",
            "EVIDENCE_PATH_ESCAPE",
            "EVIDENCE_PENDING",
            "EVIDENCE_PUBLICATION_FAILURE",
            "EVIDENCE_READ_FAILURE",
            "EVIDENCE_STEP_UNBOUND",
            "EVIDENCE_SYMLINK",
            "EVIDENCE_UNDECLARED_FILE",
            "LEASE_OWNERSHIP_UNCERTAIN",
            "OUTPUT_LIMIT_EXCEEDED",
            "QUALITY_BLOCKED",
            "READER_DEADLINE_EXCEEDED",
            "RESULT_PARSE_FAILURE",
            "RESULT_PARTIAL",
            "RESULT_ZERO",
            "SCHEDULER_FAILURE",
            "SECRET_SCAN_FAILURE",
            "SERVICE_LEASE_FAILURE",
            "SERVICE_SETUP_FAILURE",
            "STEP_CANCELLED",
            "STEP_EXECUTION_FAILURE",
            "STEP_FAILED",
            "STEP_SKIPPED",
            "STEP_TIMEOUT",
            "WEBHOOK_DESTINATION_DENIED",
            "WEBHOOK_REDIRECT_DENIED",
        ];
        let schema = serde_json::to_value(schemars::schema_for!(FailureCode)).unwrap();
        let mut declared: Vec<&str> = schema["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap())
            .collect();
        declared.sort_unstable();
        assert_eq!(declared, expected);
        for name in expected {
            let code = FailureCode::try_from(name).unwrap();
            assert_eq!(code.to_string(), name);
            assert_eq!(serde_json::to_value(code).unwrap(), name);
        }
        assert!(FailureCode::try_from("future-code").is_err());
    }

    #[test]
    fn retry_class_wire_names_are_stable_and_closed() {
        assert_eq!(RetryClass::try_from("timeout"), Ok(RetryClass::Timeout));
        assert!(RetryClass::try_from("future-retry").is_err());
    }
}
