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
}
