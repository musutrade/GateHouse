mod config;
mod errors;
mod log_parser;
mod model;
mod report;
mod runner;
mod scanner;
#[cfg(test)]
mod tests;

pub use errors::AuditError;
pub use model::AuditOutcome;
pub use runner::{parse_logs, run};

use config::{
    AllowlistEntry, BlockCommentSyntax, CommentSyntax, Config, EngineConfig, HardRule, StringSyntax,
};
use model::{ArchViolation, Violation};

#[cfg(test)]
use config::{
    parse_audit_config, validate_audit_config, ArchRule, PathsConfig, AUDIT_CONFIG_VERSION,
};
#[cfg(all(test, target_os = "linux"))]
use scanner::scan_arch_rules;
#[cfg(test)]
use scanner::{compile_regexes, is_allowlisted, scan_files};
