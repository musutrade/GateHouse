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
    code: FailureCode,
    title: &'static str,
    help: &'static str,
    documentation: String,
}

const EXPLANATIONS: &[(&str, &str, &str)] = &[
    ("WEBHOOK_DESTINATION_DENIED", "URL host or resolved address is outside the egress policy", "Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks."),
    ("WEBHOOK_REDIRECT_DENIED", "Webhook returned a redirect while redirects are disabled", "Check the configured destination and egress policy. Use an approved endpoint; do not disable address or redirect checks."),
    ("LEASE_OWNERSHIP_UNCERTAIN", "Lease identity or renewal cannot be proven", "Inspect the recorded owner and runtime identity. Use cleanup --dry-run to inspect stale leases; retain uncertain resources rather than deleting by name."),
    ("SERVICE_SETUP_FAILURE", "Service configuration or startup setup failed", "Inspect the service setup log and required tools or credentials. Confirm resource ownership before cleanup, then retry the original verification."),
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
    ("EVIDENCE_MISSING", "A required artifact or manifest entry is missing", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_UNDECLARED_FILE", "A file was found in the evidence root without a declaration", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_SYMLINK", "A symlink was found where a regular evidence file was required", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_INVALID_TYPE", "An evidence entry has an unsupported type or shape", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_READ_FAILURE", "An evidence file could not be read safely", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
    ("EVIDENCE_INVALID_METADATA", "Evidence metadata is malformed or inconsistent", "Inspect the affected artifact binding and invocation identity. Correct the producer or path and generate fresh evidence; do not edit manifests to make stale or missing evidence pass."),
];

pub(crate) fn explain(code: &str, json: bool) -> anyhow::Result<bool> {
    let parsed = FailureCode::try_from(code).map_err(|_| {
        anyhow::anyhow!(
            "unknown machine failure code; see docs/failure-codes.md for supported codes"
        )
    })?;
    let (_, title, help) = EXPLANATIONS
        .iter()
        .find(|entry| entry.0 == code)
        .ok_or_else(|| anyhow::anyhow!("registered failure code has no explanation"))?;
    let explanation = Explanation {
        code: parsed,
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
