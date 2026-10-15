use super::model::*;
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

pub(super) fn import(document: &Value, source: usize) -> Result<Vec<Finding>> {
    ensure!(document["version"] == "2.1.0", "unsupported SARIF version");
    let runs = document["runs"].as_array().context("missing SARIF runs")?;
    ensure!(!runs.is_empty(), "SARIF has no runs");
    let mut findings = Vec::new();
    for (run_index, run) in runs.iter().enumerate() {
        let driver = &run["tool"]["driver"];
        let tool = nonempty(driver, "name")?;
        let version = driver["semanticVersion"]
            .as_str()
            .or_else(|| driver["version"].as_str())
            .context("missing SARIF tool version")?;
        validate_identifier(&tool)?;
        validate_identifier(version)?;
        if let Some(invocations) = run.get("invocations") {
            let invocations = invocations
                .as_array()
                .context("invalid SARIF invocations")?;
            ensure!(
                invocations.iter().all(|v| v["executionSuccessful"] == true),
                "SARIF tool execution incomplete or failed"
            );
        }
        let results = run["results"]
            .as_array()
            .context("missing SARIF results; not an empty successful run")?;
        for (result_index, result) in results.iter().enumerate() {
            findings.push(result_finding(
                result,
                driver,
                &tool,
                version,
                source,
                format!("/runs/{run_index}/results/{result_index}"),
            )?);
            ensure!(
                findings.len() <= super::MAX_FINDINGS,
                "SARIF finding limit exceeded"
            );
        }
    }
    Ok(findings)
}

fn result_finding(
    result: &Value,
    driver: &Value,
    tool: &str,
    version: &str,
    source: usize,
    pointer: String,
) -> Result<Finding> {
    ensure!(
        result
            .get("kind")
            .is_none_or(|k| k == "fail" || k == "review"),
        "unsupported SARIF result kind"
    );
    let rule = nonempty(result, "ruleId")?;
    let descriptor = driver["rules"]
        .as_array()
        .and_then(|rules| rules.iter().find(|r| r["id"] == rule));
    if let Some(index) = result.get("ruleIndex") {
        let index = index
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .context("invalid SARIF ruleIndex")?;
        ensure!(
            driver["rules"].get(index).is_some_and(|r| r["id"] == rule),
            "SARIF ruleIndex/ruleId mismatch"
        );
    }
    let severity = match result.get("level") {
        Some(value) => value.as_str().context("invalid SARIF level")?,
        None => match descriptor.and_then(|d| d["defaultConfiguration"].get("level")) {
            Some(value) => value.as_str().context("invalid SARIF default level")?,
            None => "warning",
        },
    }
    .to_string();
    ensure!(
        matches!(severity.as_str(), "error" | "warning" | "note" | "none"),
        "unknown SARIF level"
    );
    let values = result["locations"]
        .as_array()
        .context("missing SARIF locations")?;
    ensure!(!values.is_empty(), "SARIF result has no location");
    let locations: Vec<_> = values
        .iter()
        .map(|v| {
            let physical = &v["physicalLocation"];
            let artifact = &physical["artifactLocation"];
            ensure!(
                artifact.get("uriBaseId").is_none(),
                "SARIF uriBaseId requires explicit repository-relative normalization"
            );
            let path = decode_path(&nonempty(artifact, "uri")?)?;
            let line = physical["region"]["startLine"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .context("missing/invalid SARIF startLine")?;
            let end_line = match physical["region"].get("endLine") {
                Some(value) => value
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .context("invalid SARIF endLine")?,
                None => line,
            };
            let location = Location {
                path,
                line,
                end_line,
            };
            validate_location(&location)?;
            Ok(location)
        })
        .collect::<Result<Vec<_>>>()?;
    let namespace = result["properties"]["harnessGateNamespace"]
        .as_str()
        .unwrap_or(tool)
        .to_string();
    ensure!(!namespace.trim().is_empty(), "empty identity namespace");
    let anchor = result["partialFingerprints"]["harnessGate/v1"]
        .as_str()
        .unwrap_or_default();
    let stable = !anchor.trim().is_empty();
    let anchor = if stable {
        anchor.to_string()
    } else {
        format!("location:{}", digest(&serde_json::to_vec(&locations)?))
    };
    for field in [namespace.as_str(), rule.as_str(), anchor.as_str()] {
        validate_identifier(field)?;
    }
    Ok(Finding {
        id: identity(&namespace, &rule, &anchor),
        stable,
        namespace,
        rule,
        anchor,
        tool: tool.into(),
        tool_version: version.into(),
        severity,
        message: nonempty(&result["message"], "text")?,
        locations,
        source,
        pointer,
        fixes: result["properties"]
            .get("harnessGateFixes")
            .unwrap_or(&result["fixes"])
            .clone(),
    })
}

pub(super) fn export(report: &Report) -> Value {
    // Separate runs retain exact tool/version provenance. Raw source documents
    // remain in the normalized report; export carries their explicit binding.
    let runs: Vec<_> = report.findings.iter().map(|f| {
        let locations: Vec<_> = f.locations.iter().map(|l| json!({"physicalLocation": {
            "artifactLocation": {"uri": encode_path(&l.path)}, "region": {"startLine": l.line, "endLine": l.end_line}
        }})).collect();
        let mut result = json!({"ruleId": f.rule, "level": f.severity,
            "message": {"text": f.message}, "locations": locations,
            "properties": {"harnessGateNamespace": f.namespace, "harnessGateId": f.id,
                "measurementId": report.sources[f.source].measurement_id,
                "sourcePointer": f.pointer}});
        if f.stable { result["partialFingerprints"] = json!({"harnessGate/v1": f.anchor}); }
        if !f.fixes.is_null() { result["properties"]["harnessGateFixes"] = f.fixes.clone(); }
        json!({"tool": {"driver": {"name": f.tool, "version": f.tool_version}},
            "invocations": [{"executionSuccessful": true}], "results": [result],
            "properties": {"harnessGateContext": report.context, "coreReportSha256": report.report_sha256, "coreStatus": report.core_status}})
    }).collect();
    let runs = if runs.is_empty() {
        vec![
            json!({"tool": {"driver": {"name": "harness-gate-findings", "version": "1"}}, "results": [], "invocations": [{"executionSuccessful": true}]}),
        ]
    } else {
        runs
    };
    json!({"version": "2.1.0", "$schema": "https://json.schemastore.org/sarif-2.1.0.json", "runs": runs})
}

fn encode_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn decode_path(uri: &str) -> Result<String> {
    ensure!(
        !uri.contains([':', '?', '#', '\\']),
        "SARIF location must be a relative URI path"
    );
    let mut bytes = Vec::new();
    let mut input = uri.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let hi = input
                .next()
                .and_then(|c| (c as char).to_digit(16))
                .context("invalid URI escape")?;
            let lo = input
                .next()
                .and_then(|c| (c as char).to_digit(16))
                .context("invalid URI escape")?;
            let decoded = (hi * 16 + lo) as u8;
            ensure!(decoded != b'/' && decoded != b'\\', "encoded URI separator");
            bytes.push(decoded);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).context("SARIF URI is not UTF-8")
}
