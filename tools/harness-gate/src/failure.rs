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
