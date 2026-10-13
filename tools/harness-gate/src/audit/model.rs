use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(super) struct Violation {
    pub(super) file: PathBuf,
    pub(super) line: usize,
    pub(super) content: String,
    pub(super) rule_name: String,
}
#[derive(Debug, Clone)]
pub(super) struct ArchViolation {
    pub(super) file: PathBuf,
    pub(super) line: usize,
    pub(super) content: String,
    pub(super) rule_name: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct AuditOutcome {
    pub total_violations: usize,
    pub blocker_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub report_file: PathBuf,
}
