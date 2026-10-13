//! Offline import: never resolve environment values or execute project commands.
use super::{diagnostic::SourceMap, FlowConfig};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use toml::Value;

pub(crate) const RUNTIME_BLOCKERS: &[&str] = &[
    "Global ARC_FLOW_REPORTS, AUDITOR_CONFIG, ARC_FLOW_AUDIT_CONFIG and ARC_FLOW_SECRETS_CONFIG overrides are not migrated; Harness-Gate protects audit policy and uses its own environment interface. REPORT_DIR is only a deprecated compatibility alias.",
    "Hook/prelude staged-versus-working-tree behavior, service lifecycle and runtime outcomes require shadow parity; existing secret/audit policy files and CI/hook commands remain project-owned prerequisites.",
    "Route the destination config through project scope and integrate CI separately. No ci profile, quality policy or authority transfer is invented by import.",
];

pub(crate) struct ExecutionImport {
    pub source: String,
    pub report: Json,
}

pub(crate) fn import_execution(source: &str) -> Result<ExecutionImport> {
    let original: Value =
        toml::from_str(source).context("parse Arc-Flow execution configuration")?;
    check_keys(
        &original,
        &[
            "version", "project", "paths", "policy", "doctor", "services", "parsers", "scope",
            "steps",
        ],
        "$",
    )?;
    if let Some(policy) = original.get("policy") {
        check_keys(policy, &["required_steps"], "$.policy")?;
    }
    if let Some(services) = original.get("services").and_then(Value::as_table) {
        for (id, service) in services {
            let allowed: &[&str] = match service.get("kind").and_then(Value::as_str) {
                Some("docker") => &[
                    "kind",
                    "image",
                    "image_env",
                    "external_env",
                    "inject_env",
                    "external_value_policy",
                    "startup_timeout_secs",
                    "timeout_env",
                    "container_port",
                    "environment",
                    "healthcheck",
                    "connection",
                ],
                Some("environment") => &["kind", "source_env", "inject_env"],
                _ => bail!("unsupported Arc-Flow service kind at $.services.{id}"),
            };
            check_keys(service, allowed, &format!("$.services.{id}"))?;
        }
    }
    if let Some(parsers) = original.get("parsers").and_then(Value::as_table) {
        for (id, parser) in parsers {
            if parser.get("kind").and_then(Value::as_str) != Some("regex") {
                bail!("unsupported Arc-Flow parser kind at $.parsers.{id}");
            }
            check_keys(
                parser,
                &["kind", "patterns", "capture", "minimum"],
                &format!("$.parsers.{id}"),
            )?;
        }
    }
    // Arc-Flow reads TOML literally. Harness-Gate interpolates basic strings.
    // Reject even ambiguous/comment occurrences instead of capturing host secrets.
    if source.contains("${") {
        bail!("unsupported Arc-Flow literal `${{...}}`: Harness-Gate environment interpolation could change its meaning");
    }
    let mut migrated = original.clone();
    let paths = migrated
        .get_mut("paths")
        .and_then(Value::as_table_mut)
        .context("missing paths table")?;
    let materialized_secrets_default = !paths.contains_key("secrets_config");
    if materialized_secrets_default {
        paths.insert(
            "secrets_config".into(),
            Value::String(".arc-flow/secrets.toml".into()),
        );
    }
    let steps = migrated
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .context("missing steps array")?;
    let mut repository_inputs = 0;
    for step in steps {
        check_keys(
            step,
            &[
                "id",
                "label",
                "component",
                "profiles",
                "program",
                "args",
                "cwd",
                "log",
                "timeout_secs",
                "timeout_env",
                "parser",
                "services",
                "remove_env",
                "depends_on",
            ],
            "$.steps[]",
        )?;
        let table = step.as_table_mut().context("step must be a table")?;
        for required in [
            "id",
            "label",
            "component",
            "profiles",
            "program",
            "args",
            "cwd",
            "log",
            "timeout_secs",
        ] {
            if !table.contains_key(required) {
                bail!("Arc-Flow step is missing required field {required}");
            }
        }
        if !table.contains_key("input") {
            table.insert("input".into(), Value::String("repository".into()));
            repository_inputs += 1;
        }
    }
    // Deliberately bypass the runtime loader: host environment must not influence
    // import bytes, timeouts, policy paths or diagnostics.
    let config: FlowConfig = migrated
        .clone()
        .try_into()
        .context("unsupported execution mapping")?;
    let output = toml::to_string_pretty(&migrated)?;
    config
        .validate_with_diagnostics(&SourceMap::from_source(&output), None, None)
        .context("imported execution configuration is invalid")?;
    let typed = Value::try_from(&config).context("serialize validated execution configuration")?;
    preserve_fields(&migrated, &typed, "$")?;
    let step_inventory: Vec<Json> = config
        .steps
        .iter()
        .map(|step| {
            json!({
                "id": step.id,
                "component": step.component,
                "profiles": step.profiles,
                "blocks_when_selected": true,
                "listed_in_required_steps": config.policy.required_steps.contains(&step.id),
                "depends_on": step.depends_on,
            })
        })
        .collect();
    let report = json!({
        "version": 1,
        "status": "execution-config-imported",
        "authority_transfer": "blocked",
        "runtime_blockers": RUNTIME_BLOCKERS,
        "source_sha256": format!("{:x}", Sha256::digest(source.as_bytes())),
        "output_sha256": format!("{:x}", Sha256::digest(output.as_bytes())),
        "preserved_sections": ["project", "paths", "policy", "doctor", "services", "parsers", "scope", "steps"],
        "steps": step_inventory,
        "transformations": {
            "materialized_arc_flow_secrets_default": materialized_secrets_default,
            "explicit_repository_inputs": repository_inputs,
        },
        "ux_metrics": {
            "import_commands": 1,
            "manually_reentered_steps": 0,
            "manual_execution_config_edits": 0,
            "source_config_bytes": source.len(),
            "generated_config_bytes": output.len(),
            "retained_source_configs": 1,
            "generated_duplicate_configs": 1,
            "duplicated_step_definitions": config.steps.len(),
            "preserved_source_scalar_values": scalar_count(&original),
            "operator_elapsed_seconds": null,
            "runtime_integration_effort": "unmeasured; runtime blockers remain",
        },
    });
    Ok(ExecutionImport {
        source: output,
        report,
    })
}

fn check_keys(value: &Value, allowed: &[&str], path: &str) -> Result<()> {
    for key in value
        .as_table()
        .with_context(|| format!("{path} must be a table"))?
        .keys()
    {
        if !allowed.contains(&key.as_str()) {
            bail!("unsupported Arc-Flow mapping: {path}.{key}");
        }
    }
    Ok(())
}

// A recursive projection check catches serde flatten/enum fields that a decoder
// might ignore. Compare arrays in order except the schema's set-valued fields.
fn preserve_fields(source: &Value, target: &Value, path: &str) -> Result<()> {
    match (source, target) {
        (Value::Table(source), Value::Table(target)) => {
            for (key, value) in source {
                let path = format!("{path}.{key}");
                let mapped = target
                    .get(key)
                    .with_context(|| format!("lossy mapping: {path} was dropped"))?;
                preserve_fields(value, mapped, &path)?;
            }
        }
        (Value::Array(source), Value::Array(target)) if source.len() == target.len() => {
            if path.ends_with(".profiles") || path.ends_with(".components") {
                let mut left: Vec<_> = source.iter().map(Value::to_string).collect();
                let mut right: Vec<_> = target.iter().map(Value::to_string).collect();
                left.sort();
                right.sort();
                if left != right {
                    bail!("lossy mapping at {path}");
                }
            } else {
                for (index, (left, right)) in source.iter().zip(target).enumerate() {
                    preserve_fields(left, right, &format!("{path}[{index}]"))?;
                }
            }
        }
        _ if source == target => {}
        _ => bail!("lossy mapping at {path}"),
    }
    Ok(())
}

fn scalar_count(value: &Value) -> usize {
    match value {
        Value::Table(table) => table.values().map(scalar_count).sum(),
        Value::Array(array) => array.iter().map(scalar_count).sum(),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_checks_generic_resource_conflicts_without_changing_serial_flows() {
        let source = r#"version = 2
[project]
name = "resource-fixture"
default_profile = "full"
hook_profile = "full"
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
[scope]
unmatched = "fail"
rules = [{ patterns = ["**"], components = ["custom"] }]
[[steps]]
id = "one"
label = "One"
component = "custom"
program = "custom-runner"
args = []
cwd = "{root}"
timeout_secs = 5
profiles = ["full"]
log = "shared.log"
[[steps]]
id = "two"
label = "Two"
component = "custom"
program = "custom-runner"
args = []
cwd = "{root}"
timeout_secs = 5
profiles = ["full"]
log = "shared.log"
"#;
        let error = import_execution(source).err().unwrap();
        assert!(
            format!("{error:#}").contains("HGCFG-DUPLICATE-LOG"),
            "{error:#}"
        );
        let valid = source.replacen("log = \"shared.log\"", "log = \"one.log\"", 1);
        let imported = import_execution(&valid).unwrap();
        FlowConfig::from_source(&imported.source).unwrap();
        assert_eq!(imported.report["steps"].as_array().unwrap().len(), 2);
    }
}
