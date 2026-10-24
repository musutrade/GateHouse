//! Read-only repair plans bound to original TOML bytes, not display locations.
mod apply;
use super::diagnostic::{ConfigCheckReport, ConfigDiagnostic, ConfigDiagnostics};
use super::{read_config_source, resolve_config_path, FlowConfig};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Applicability {
    Manual,
    MaybeIncorrect,
}

#[derive(Serialize)]
struct TextEdit {
    start: usize,
    end: usize,
    replacement: String,
}

#[derive(Serialize)]
struct Suggestion {
    diagnostic_id: String,
    field: String,
    message: String,
    applicability: Applicability,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit: Option<TextEdit>,
}

#[derive(Serialize)]
struct Plan {
    schema: &'static str,
    scope: &'static str,
    source_sha256: String,
    configuration: ConfigCheckReport,
    suggestions: Vec<Suggestion>,
}

#[derive(Default, Deserialize)]
struct SourceView {
    #[serde(default)]
    steps: Vec<StepView>,
    #[serde(default)]
    parsers: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
struct StepView {
    id: Option<String>,
    kind: Option<String>,
    log: Option<toml::Spanned<String>>,
}

pub(crate) fn run(
    root: &Path,
    override_path: Option<PathBuf>,
    json: bool,
    apply: bool,
) -> Result<bool> {
    if apply {
        anyhow::ensure!(!json, "interactive repairs cannot use JSON output");
        return self::apply::run(root, override_path);
    }
    let root = root.canonicalize().context("resolve project root")?;
    let path = resolve_config_path(&root, override_path)?;
    let source = read_config_source(&path)?;
    let plan = plan(&source, &path, &root);
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!("Workflow repair suggestions (read-only; not full config check):");
        for suggestion in &plan.suggestions {
            println!(
                "{} at {}: {}",
                safe_text(&suggestion.diagnostic_id),
                safe_text(&suggestion.field),
                safe_text(&suggestion.message),
            );
            let applicability = match suggestion.applicability {
                Applicability::Manual => "manual",
                Applicability::MaybeIncorrect => "maybe incorrect; review intended behavior",
            };
            println!("  applicability: {applicability}");
        }
        if plan.suggestions.is_empty() {
            println!("No workflow repairs suggested; run config check for full validation.");
        }
    }
    Ok(plan.configuration.valid)
}

fn plan(source: &str, path: &Path, root: &Path) -> Plan {
    let configuration =
        match FlowConfig::from_source_with_diagnostics(source, Some(path), Some(root)) {
            Ok(_) => ConfigDiagnostics::empty().report(),
            Err(diagnostics) => diagnostics.report(),
        };
    let view = toml::from_str::<SourceView>(source).unwrap_or_default();
    let suggestions = configuration
        .diagnostics
        .iter()
        .map(|diagnostic| suggest(diagnostic, &view, source))
        .collect();
    Plan {
        schema: "configuration-suggestions/v1",
        scope: "workflow-only",
        source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        configuration,
        suggestions,
    }
}

fn safe_text(value: &str) -> String {
    super::diagnostic::terminal_text(value)
}

fn suggest(diagnostic: &ConfigDiagnostic, view: &SourceView, source: &str) -> Suggestion {
    let mut suggestion = Suggestion {
        diagnostic_id: diagnostic.id().to_owned(),
        field: diagnostic.path.clone(),
        message: diagnostic.help.clone(),
        applicability: Applicability::Manual,
        edit: None,
    };
    match diagnostic.id() {
        "HGCFG-DUPLICATE-LOG" | "HGCFG-INVALID-LOG" => {
            suggest_log(&mut suggestion, view, source);
        }
        "HGCFG-UNKNOWN-REFERENCE" if diagnostic.path.ends_with(".parser") => {
            suggestion.message = format!(
                "Choose the intended declared parser; available identifiers: {}",
                identifiers(view.parsers.keys().map(String::as_str)),
            );
        }
        "HGCFG-DEPENDENCY" | "HGCFG-UNKNOWN-REFERENCE"
            if diagnostic.path.ends_with(".depends_on")
                || diagnostic.path == "policy.required_steps" =>
        {
            suggestion.message = format!(
                "Repair the intended reference without dropping required steps. Declared step identifiers: {}",
                identifiers(view.steps.iter().filter_map(|step| step.id.as_deref())),
            );
        }
        "HGCFG-DUPLICATE-FIELD" => {
            suggestion.message = format!(
                "Review {} for duplicate declarations; rename only with all references updated. Do not remove a required check.",
                safe_text(&diagnostic.path),
            );
        }
        "HGCFG-INVALID-ENVIRONMENT" => {
            suggestion.message = format!(
                "Choose an uppercase identifier for {} and update its consumers together; changing a name alone can break service injection.",
                safe_text(&diagnostic.path),
            );
        }
        _ => {}
    }
    suggestion
}

fn identifiers<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let items = values.take(8).map(safe_text).collect::<Vec<_>>();
    if items.is_empty() {
        "none".into()
    } else {
        items.join(", ")
    }
}

fn suggest_log(suggestion: &mut Suggestion, view: &SourceView, source: &str) {
    let Some(index) = suggestion
        .field
        .strip_prefix("steps[")
        .and_then(|field| field.strip_suffix("].log"))
        .and_then(|index| index.parse::<usize>().ok())
    else {
        return;
    };
    let Some(step) = view.steps.get(index) else {
        return;
    };
    if step.kind.as_deref() == Some("builtin-gate") {
        return;
    }
    let Some(log) = &step.log else {
        return;
    };
    if log.get_ref().contains("${") {
        return;
    }
    let name = format!("step-{index}-repaired.log");
    if view
        .steps
        .iter()
        .filter(|step| step.kind.as_deref() != Some("builtin-gate"))
        .filter_map(|step| step.log.as_ref())
        .any(|log| log.get_ref().contains("${") || log.get_ref().eq_ignore_ascii_case(&name))
    {
        return;
    }
    let span = log.span();
    if source.get(span.clone()).is_none() {
        return;
    }
    suggestion.message =
        format!("Use {name} for this step; review any tooling that consumes its log filename.");
    suggestion.applicability = Applicability::MaybeIncorrect;
    suggestion.edit = Some(TextEdit {
        start: span.start,
        end: span.end,
        replacement: format!("\"{name}\""),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(id: &str, field: &str) -> ConfigDiagnostic {
        ConfigDiagnostics::synthetic(id, field, "invalid field", "repair intended declaration")
            .report()
            .diagnostics
            .remove(0)
    }

    #[test]
    fn log_candidates_preserve_surrounding_original_bytes() {
        for token in ["'日志.log'", "\"bad/path.log\"", "\"\"\"same.log\"\"\""] {
            let source = format!("# context\n[[steps]]\nlog = {token} # retained\n");
            let view: SourceView = toml::from_str(&source).unwrap();
            let suggestion = suggest(
                &diagnostic("HGCFG-INVALID-LOG", "steps[0].log"),
                &view,
                &source,
            );
            assert!(matches!(
                suggestion.applicability,
                Applicability::MaybeIncorrect
            ));
            let edit = suggestion.edit.unwrap();
            assert_eq!(&source[edit.start..edit.end], token);
            let mut replacement = source.clone();
            replacement.replace_range(edit.start..edit.end, &edit.replacement);
            assert_eq!(
                replacement,
                "# context\n[[steps]]\nlog = \"step-0-repaired.log\" # retained\n"
            );
            let parsed: SourceView = toml::from_str(&replacement).unwrap();
            assert_eq!(
                parsed.steps[0].log.as_ref().unwrap().get_ref(),
                "step-0-repaired.log"
            );
        }
    }

    #[test]
    fn log_candidates_reject_ambiguous_or_semantic_boundaries() {
        for (source, field) in [
            ("[[steps]]\nlog = 'bad/path.log'\n[[steps]]\nlog = \"${PEER_LOG:-step-0-repaired.log}\"", "steps[0].log"),
            ("[[steps]]\nlog = 'same.log'", "steps"),
            ("[[steps]]\nlog = 'same.log'", "steps[x].log"),
            ("[[steps]]\nlog = 'same.log'", "steps[8].log"),
            ("[[steps]]\nid = 'step'", "steps[0].log"),
            (
                "[[steps]]\nkind = 'builtin-gate'\nlog = 'same.log'",
                "steps[0].log",
            ),
            ("[[steps]]\nlog = '${LOG:-same.log}'", "steps[0].log"),
            (
                "[[steps]]\nlog = 'same.log'\n[[steps]]\nlog = 'STEP-0-REPAIRED.LOG'",
                "steps[0].log",
            ),
        ] {
            let view = toml::from_str(source).unwrap();
            let suggestion = suggest(&diagnostic("HGCFG-DUPLICATE-LOG", field), &view, source);
            assert!(suggestion.edit.is_none(), "{source} / {field}");
            assert!(matches!(suggestion.applicability, Applicability::Manual));
        }
        let view = toml::from_str("[[steps]]\nlog = 'same.log'").unwrap();
        assert!(
            suggest(&diagnostic("HGCFG-INVALID-LOG", "steps[0].log"), &view, "")
                .edit
                .is_none()
        );
    }

    #[test]
    fn reference_suggestions_respect_the_declared_namespace_and_requiredness() {
        let source = "[[steps]]\nid = 'required-check'\n[parsers.junit]\nkind = 'junit'\n";
        let view = toml::from_str(source).unwrap();
        let parser = suggest(
            &diagnostic("HGCFG-UNKNOWN-REFERENCE", "steps[0].parser"),
            &view,
            source,
        );
        assert!(parser.message.contains("junit"));
        assert!(!parser.message.contains("required-check"));
        let required = suggest(
            &diagnostic("HGCFG-UNKNOWN-REFERENCE", "policy.required_steps"),
            &view,
            source,
        );
        assert!(required.message.contains("without dropping required steps"));
        assert!(required.message.contains("required-check") && required.edit.is_none());
        let service = suggest(
            &diagnostic("HGCFG-UNKNOWN-REFERENCE", "steps[0].services[0]"),
            &view,
            source,
        );
        assert_eq!(service.message, "repair intended declaration");
        for id in [
            "HGCFG-DEPENDENCY",
            "HGCFG-DUPLICATE-FIELD",
            "HGCFG-INVALID-ENVIRONMENT",
        ] {
            let suggestion = suggest(&diagnostic(id, "steps[0].depends_on"), &view, source);
            assert!(suggestion.edit.is_none());
            assert!(matches!(suggestion.applicability, Applicability::Manual));
        }
    }

    #[test]
    fn identifiers_are_bounded_and_terminal_safe() {
        assert_eq!(identifiers(std::iter::empty()), "none");
        let value = identifiers(["token=synthetic-secret", "name\n\u{1b}[2J"].into_iter());
        assert!(!value.contains("synthetic-secret") && !value.contains('\u{1b}'));
        assert!(value.contains("[REDACTED]"));
        assert_eq!(identifiers(std::iter::repeat("id")).split(", ").count(), 8);
    }
}
