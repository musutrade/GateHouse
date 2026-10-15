//! Pure report formatting. Publication, evidence validation and path safety remain
//! in the parent module. Keep output bytes compatible with existing consumers.

use super::super::{quality, VerificationReport};
use crate::utils::redaction::redact_text;

pub(super) fn markdown(report: &VerificationReport) -> String {
    let mut output = String::from("=== Verification report ===\n");
    output.push_str(&format!("Timestamp: {}\n", report.timestamp));
    output.push_str(&format!("Profile: {}\n", report.profile));
    output.push_str(&format!(
        "Components: {}\n\n",
        report
            .scope
            .components
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(",")
    ));
    for step in &report.steps {
        let status = if step.waived {
            "WAIVED"
        } else if step.passed {
            "PASS"
        } else {
            "FAIL"
        };
        output.push_str(&format!(
            "- {status}: {} ({} ms)",
            redact_text(&step.label),
            step.duration_ms
        ));
        if let Some(detail) = &step.detail {
            output.push_str(&format!(" - {}", redact_text(detail)));
        }
        if !step.passed && !step.waived {
            output.push_str(&format!("; log: {}", redact_text(&step.log)));
        }
        output.push('\n');
    }
    for step in &report.skipped_steps {
        output.push_str(&format!(
            "- SKIPPED: {} - {}\n",
            redact_text(&step.label),
            redact_text(&step.reason)
        ));
    }
    if let Some(quality) = &report.quality {
        output.push_str(&quality::diagnostics(quality));
    }
    output.push_str(&format!(
        "\nTEST_SUMMARY: {}\n",
        if report.passed && report.steps.iter().any(|step| step.waived) {
            "WAIVED"
        } else if report.passed {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    output
}

pub(super) fn junit(report: &VerificationReport) -> String {
    let failures = report.steps.iter().filter(|step| !step.passed).count();
    let total = report.steps.len() + report.skipped_steps.len();
    let mut output = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuite name=\"harness-gate\" tests=\"{}\" failures=\"{}\">\n", total, failures);
    for step in &report.steps {
        output.push_str(&format!(
            "  <testcase name=\"{}\" time=\"{:.3}\">",
            escape(&redact_text(&step.label)),
            step.duration_ms as f64 / 1000.0
        ));
        if !step.passed {
            output.push_str(&format!(
                "<failure message=\"{}\"/>",
                escape(&redact_text(step.detail.as_deref().unwrap_or("failed")))
            ));
        } else if step.waived {
            output.push_str("<skipped message=\"WAIVED\"/>");
        }
        output.push_str("</testcase>\n");
    }
    for step in &report.skipped_steps {
        output.push_str(&format!(
            "  <testcase name=\"{}\" time=\"0.000\"><skipped message=\"{}\"/></testcase>\n",
            escape(&redact_text(&step.label)),
            escape(&redact_text(&step.reason))
        ));
    }
    output.push_str("</testsuite>\n");
    output
}

pub(super) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
