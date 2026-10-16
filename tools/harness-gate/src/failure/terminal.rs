//! Human diagnostics from established failure codes; never evaluate policy.
use super::{FailureCode, EXPLANATIONS};

pub(crate) fn render(
    code: FailureCode,
    context: &str,
    detail: Option<&str>,
    log: Option<&str>,
) -> String {
    let code = code.to_string();
    let entry = EXPLANATIONS.iter().find(|entry| entry.0 == code);
    let (title, help) = entry.map(|entry| (entry.1, entry.2)).unwrap_or((
        "Verification failed",
        "Inspect the recorded failure and retained evidence before retrying.",
    ));
    let mut text = format!(
        "{}: {title}\n  = context: {}",
        crate::ui::error(format!("error[{code}]")),
        safe_text(context)
    );
    if let Some(detail) = detail.filter(|value| !value.is_empty()) {
        text.push_str(&format!("\n  = reason: {}", safe_text(detail)));
    }
    if let Some(log) = log.filter(|value| !value.is_empty()) {
        text.push_str(&format!("\n  = log: {}", safe_text(log)));
    }
    text.push_str(&format!(
        "\n  = help: {help}\n  = explain: harness-gate explain {code}"
    ));
    text
}

/// Summarize only the authoritative published blocker list, never gate states.
pub(crate) fn quality_blockers(report: &serde_json::Value) -> String {
    if report["aggregate"]["state"] == "pass" {
        return String::new();
    }
    let Some(blockers) = report["aggregate"]["blockers"].as_array() else {
        return String::new();
    };
    let mut lines = Vec::new();
    for blocker in blockers.iter().take(5) {
        let mut fields = Vec::new();
        for key in ["policy", "subject", "state"] {
            if let Some(value) = blocker[key].as_str() {
                fields.push(format!("{key}={}", summary_text(value)));
            }
        }
        if let Some(reason) = blocker_reason(report, blocker) {
            fields.push(format!("reason={}", summary_text(reason)));
        }
        if fields.is_empty() {
            fields.push("details unavailable; inspect the report".into());
        }
        lines.push(format!("  = blocker: {}", fields.join("; ")));
    }
    if blockers.len() > 5 {
        lines.push(format!(
            "  = additional blockers: {}; inspect the report",
            blockers.len() - 5
        ));
    }
    lines.join("\n")
}

fn blocker_reason<'a>(
    report: &'a serde_json::Value,
    blocker: &'a serde_json::Value,
) -> Option<&'a str> {
    if let Some(reason) = blocker["reason"].as_str() {
        return Some(reason);
    }
    let policy = blocker["policy"].as_str()?;
    let subject = blocker["subject"].as_str()?;
    let state = blocker["state"].as_str()?;
    let gates = report["gates"].as_object()?;
    let mut matches = gates.values().filter(|gate| {
        gate["policy"].as_str() == Some(policy)
            && gate["subject"].as_str() == Some(subject)
            && gate["state"].as_str() == Some(state)
    });
    let gate = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    gate["reason"].as_str()
}

fn summary_text(text: &str) -> String {
    let safe = safe_text(text);
    let mut characters = safe.chars();
    let mut bounded = characters.by_ref().take(256).collect::<String>();
    if characters.next().is_some() {
        bounded.push('…');
    }
    bounded
}

fn safe_text(text: &str) -> String {
    crate::utils::redaction::redact_text(text)
        .chars()
        .take(1024)
        .flat_map(|character| character.escape_debug())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_machine_code_has_terminal_guidance() {
        for &(code, title, help) in EXPLANATIONS {
            let code = FailureCode::try_from(code).unwrap();
            let rendered = render(code, "step", None, None);
            assert!(rendered.contains(title) && rendered.contains(help));
            assert!(rendered.contains(&format!("harness-gate explain {code}")));
        }
    }

    #[test]
    fn diagnostic_metadata_is_redacted_bounded_and_control_safe() {
        let rendered = render(
            FailureCode::StepFailed,
            "step\n\u{1b}[2J",
            Some("token=synthetic-secret"),
            Some("log\rname"),
        );
        assert!(!rendered.contains("synthetic-secret") && !rendered.contains('\u{1b}'));
        assert!(rendered.contains("[REDACTED]") && rendered.contains("\\n"));
        assert!(safe_text(&"x".repeat(2000)).len() <= 1024);
    }
    #[test]
    fn quality_summary_uses_only_authoritative_blockers_and_unique_reasons() {
        let mut report = serde_json::json!({"aggregate":{"state":"fail","blockers":[{"policy":"required","subject":"src","state":"fail"}]},"gates":{
            "required:src":{"policy":"required","subject":"src","state":"fail","reason":"coverage too low"},
            "optional:other":{"policy":"optional","subject":"other","state":"fail","reason":"not blocking"}
        }});
        let original = report.clone();
        let summary = quality_blockers(&report);
        assert!(
            summary.contains("policy=required; subject=src; state=fail; reason=coverage too low")
        );
        assert!(!summary.contains("optional") && !summary.contains("not blocking"));
        assert_eq!(report, original);
        report["gates"]["duplicate"] = report["gates"]["required:src"].clone();
        assert!(!quality_blockers(&report).contains("reason="));
        report["aggregate"]["state"] = serde_json::json!("pass");
        assert!(quality_blockers(&report).is_empty());
    }

    #[test]
    fn quality_summary_handles_missing_identifiers_and_bounds_untrusted_fields() {
        let report = serde_json::json!({"aggregate":{"state":"fail","blockers":[
            {"state":"measurement_error","reason":"invalid exception metadata","errors":["DO-NOT-PRINT"]},
            {"policy":"missing","subject":null,"state":"blocked"},
            {"policy":"token=synthetic-secret\n\u{1b}[2J","subject":"x".repeat(2000),"state":"fail"},
            {"policy":"mismatch","subject":"s","state":"blocked"},
            {}, {"policy":"overflow-secret"}
        ]},"gates":{"nulls":{"subject":null,"reason":"wrong-null-match"},"other-state":{"policy":"mismatch","subject":"s","state":"pass","reason":"wrong-state"}}});
        let summary = quality_blockers(&report);
        assert_eq!(summary.matches("  = blocker:").count(), 5);
        assert!(summary.contains("additional blockers: 1"));
        assert!(summary.contains("reason=invalid exception metadata"));
        assert!(summary.contains("policy=missing; state=blocked"));
        assert!(summary.contains("[REDACTED]") && summary.contains("\\n"));
        for forbidden in [
            "DO-NOT-PRINT",
            "synthetic-secret",
            "wrong-null-match",
            "wrong-state",
            "overflow-secret",
            "\u{1b}",
        ] {
            assert!(!summary.contains(forbidden), "{forbidden}");
        }
        assert!(!summary.contains(&"x".repeat(257)));
        assert!(summary.contains('…'));
        assert!(quality_blockers(&serde_json::json!({"aggregate":{"state":"fail"}})).is_empty());
    }
}
