//! Additional terminal context; persisted reports and decisions remain independent.
use super::VerificationReport;
use crate::failure::{render_terminal, FailureCode};

pub(super) fn print(report: &VerificationReport) {
    for message in report.steps.iter().filter_map(step_message) {
        eprintln!("{message}");
    }
    if let Some(message) = report
        .quality
        .as_ref()
        .and_then(|quality| quality_message(&report.profile, quality))
    {
        eprintln!("{message}");
    }
}

fn step_message(step: &crate::process::TaskResult) -> Option<String> {
    if step.passed || step.waived {
        return None;
    }
    Some(render_terminal(
        step.failure_code.unwrap_or(FailureCode::StepFailed),
        step.step_id.as_deref().unwrap_or(&step.label),
        step.detail.as_deref(),
        Some(&step.log),
    ))
}

fn quality_message(profile: &str, quality: &super::quality::QualityResult) -> Option<String> {
    if quality.passed() {
        return None;
    }
    Some(render_terminal(
        FailureCode::QualityBlocked,
        &format!("quality profile {profile} ({})", quality.phase),
        quality.error.as_deref(),
        quality.project_report_path.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn quality_context_does_not_change_decisions_or_treat_omission_as_failure() {
        let mut quality = super::super::quality::QualityResult {
            schema: "quality-verification/v1",
            status: "blocked",
            phase: "configuration",
            error: Some("token=synthetic-secret".into()),
            selection: Value::Null,
            inputs: Value::Null,
            evidence: Value::Null,
            baseline: Value::Null,
            project_report: Value::Null,
            project_report_path: Some("quality-project-report.json".into()),
            output: None,
            formats: Default::default(),
            evaluation_time: None,
            participation: json!({"profile":"full"}),
            full_quality_status: "blocked",
            producers: Value::Null,
        };
        let before = serde_json::to_value(&quality).unwrap();
        let message = quality_message("full", &quality).unwrap();
        assert!(message.contains("error[QUALITY_BLOCKED]"));
        assert!(message.contains("quality profile full (configuration)"));
        assert!(message.contains("quality-project-report.json"));
        assert!(!message.contains("synthetic-secret"));
        assert_eq!(serde_json::to_value(&quality).unwrap(), before);
        quality.status = "pass";
        assert!(quality_message("full", &quality).is_none());
        quality.status = "not_collected";
        quality.phase = "complete";
        assert!(quality_message("hook", &quality).is_none());
    }
    #[test]
    fn waived_and_successful_steps_do_not_become_terminal_failures() {
        let mut step = crate::process::TaskResult {
            cache: None,
            step_id: None,
            invocation_id: None,
            attempt: None,
            started_at: None,
            finished_at: None,
            label: "legacy step".into(),
            passed: false,
            timed_out: false,
            cancelled: false,
            duration_ms: 0,
            log: String::new(),
            detail: None,
            failure_code: None,
            attempts: Vec::new(),
            flaky: false,
            retry_class: None,
            parser: None,
            waived: false,
            waiver: None,
            runner: None,
        };
        let message = step_message(&step).unwrap();
        assert!(message.contains("error[STEP_FAILED]") && message.contains("legacy step"));
        assert!(!message.contains("= log:"));
        step.waived = true;
        assert!(step_message(&step).is_none());
        step.waived = false;
        step.passed = true;
        assert!(step_message(&step).is_none());
    }
}
