use super::{ArchViolation, Config, Violation};
use crate::utils::redaction::redact_text;
use serde::Serialize;

pub(super) fn generate_markdown(
    config: &Config,
    hard_violations: &[Violation],
    arch_violations: &[ArchViolation],
) -> String {
    let mut output = String::new();
    let occurrence_limit = config.engine.markdown_occurrences_per_rule;

    output.push_str("=== 【自动化硬性约束扫描结果】 ===\n\n");

    for rule in &config.hard_rules {
        let rule_violations: Vec<&Violation> = hard_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();

        let count = rule_violations.len();
        output.push_str(&format!(
            ">> {}: 违规数量 {}\n",
            redact_text(&rule.name),
            count
        ));

        if count > 0 {
            for v in rule_violations.iter().take(occurrence_limit) {
                output.push_str(&format!(
                    "    {}:{}: {}\n",
                    redact_text(&v.file.to_string_lossy()),
                    v.line,
                    redact_text(&v.content)
                ));
            }
            if count > occurrence_limit {
                output.push_str(&format!("    ... 剩余 {} 处\n", count - occurrence_limit));
            }
        } else {
            output.push_str("  ✅ 未发现\n");
        }
        output.push('\n');
    }

    output.push_str("=== 【架构分层违规预扫描】 ===\n\n");

    for rule in &config.arch_rules {
        let violations: Vec<&ArchViolation> = arch_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();
        let count = violations.len();

        output.push_str(&format!(
            ">> {}: 违规数量 {}\n",
            redact_text(&rule.name),
            count
        ));

        if count > 0 {
            for v in violations.iter().take(occurrence_limit) {
                output.push_str(&format!(
                    "    {}:{}: {}\n",
                    redact_text(&v.file.to_string_lossy()),
                    v.line,
                    redact_text(&v.content)
                ));
            }
            if count > occurrence_limit {
                output.push_str(&format!("    ... 剩余 {} 处\n", count - occurrence_limit));
            }
            output.push_str(&format!("  💡 建议: {}\n", redact_text(&rule.suggestion)));
        } else {
            output.push_str("  ✅ 未发现违规\n");
        }
        output.push('\n');
    }

    output
}

#[derive(Debug, Serialize)]
pub(super) struct JsonOccurrence {
    file: String,
    line: usize,
    content: String,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonViolation {
    pub(super) rule: String,
    pub(super) severity: String,
    pub(super) count: usize,
    pub(super) occurrences: Vec<JsonOccurrence>,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonArchViolation {
    rule: String,
    layer: String,
    count: usize,
    suggestion: String,
    occurrences: Vec<JsonOccurrence>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct JsonSummary {
    pub(super) total_violations: usize,
    pub(super) blocker_count: usize,
    pub(super) error_count: usize,
    pub(super) warning_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonReport {
    pub(super) timestamp: String,
    pub(super) hard_violations: Vec<JsonViolation>,
    pub(super) arch_violations: Vec<JsonArchViolation>,
    pub(super) summary: JsonSummary,
}

pub(super) fn generate_report(
    config: &Config,
    hard_violations: &[Violation],
    arch_violations: &[ArchViolation],
) -> JsonReport {
    let mut hard_json = Vec::new();
    for rule in &config.hard_rules {
        let rule_violations: Vec<&Violation> = hard_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();

        let occurrences: Vec<JsonOccurrence> = rule_violations
            .iter()
            .map(|v| JsonOccurrence {
                file: redact_text(&v.file.to_string_lossy()),
                line: v.line,
                content: redact_text(&v.content),
            })
            .collect();

        hard_json.push(JsonViolation {
            rule: redact_text(&rule.name),
            severity: redact_text(&rule.severity),
            count: occurrences.len(),
            occurrences,
        });
    }

    let mut arch_json = Vec::new();
    for rule in &config.arch_rules {
        let rule_violations: Vec<&ArchViolation> = arch_violations
            .iter()
            .filter(|v| v.rule_name == rule.name)
            .collect();

        let occurrences: Vec<JsonOccurrence> = rule_violations
            .iter()
            .map(|v| JsonOccurrence {
                file: redact_text(&v.file.to_string_lossy()),
                line: v.line,
                content: redact_text(&v.content),
            })
            .collect();

        arch_json.push(JsonArchViolation {
            rule: redact_text(&rule.name),
            layer: redact_text(&rule.layer),
            count: occurrences.len(),
            suggestion: redact_text(&rule.suggestion),
            occurrences,
        });
    }

    let total: usize = hard_json.iter().map(|v| v.count).sum::<usize>()
        + arch_json.iter().map(|v| v.count).sum::<usize>();
    let blocker_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "blocker")
        .map(|v| v.count)
        .sum();
    let error_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "error")
        .map(|v| v.count)
        .sum();
    let warning_count: usize = hard_json
        .iter()
        .filter(|v| v.severity == "warning")
        .map(|v| v.count)
        .sum();

    JsonReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        hard_violations: hard_json,
        arch_violations: arch_json,
        summary: JsonSummary {
            total_violations: total,
            blocker_count,
            error_count,
            warning_count,
        },
    }
}
