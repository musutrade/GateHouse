//! Non-authoritative replay transport and field-level comparison. No collectors.
use super::{error, evidence, policy, project_report, require, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

fn context(value: &Value) -> Result<evidence::ValidationContext<'_>> {
    for key in ["project", "expected", "records"] {
        require(
            value.get(key).is_some(),
            format!("missing replay context: {key}"),
        )?;
    }
    let path = |key| {
        value[key]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(Path::new)
            .ok_or_else(|| error(format!("missing replay context: {key}")))
    };
    Ok(evidence::ValidationContext {
        project: &value["project"],
        expected: &value["expected"],
        source_root: path("source_root")?,
        artifact_root: path("artifact_root")?,
    })
}

fn failure(e: super::Error) -> Value {
    json!({"reason_class": format!("{:?}", e.class), "reason": e.message})
}

/// Evaluate only caller-provided evidence and context, including a fixed clock.
pub fn evaluate(case: &Value) -> Result<Value> {
    require(case.get("policy").is_some(), "missing replay policy")?;
    let now = case["now"]
        .as_str()
        .ok_or_else(|| error("missing replay clock"))?;
    chrono::DateTime::parse_from_rfc3339(now).map_err(|_| error("invalid replay clock"))?;
    let head = context(&case["head"])?;
    let base = case.get("base").map(context).transpose()?;
    let mut output = json!({});
    output["validation"] = match evidence::validate_evidence(&case["head"]["records"], &head) {
        Ok(_) => json!({"accepted": true}),
        Err(e) => {
            let mut value = failure(e);
            value["accepted"] = json!(false);
            value
        }
    };
    let result = policy::evaluate(
        &case["policy"],
        &case["head"]["records"],
        &head,
        &policy::EvaluationOptions {
            selection: case.get("selection"),
            base_records: case.get("base").map(|v| &v["records"]),
            base_context: base.as_ref(),
            mappings: case.get("mappings"),
            exceptions: case.get("exceptions"),
            now: Some(now),
        },
    );
    match result {
        Ok(result) => match project_report::report(&result, head.project, &case["policy"]) {
            Ok(report) => {
                output["policy_result"] = result;
                output["project_report"] = report;
            }
            Err(e) => {
                output["evaluation_error"] = failure(e);
            }
        },
        Err(e) => {
            output["evaluation_error"] = failure(e);
        }
    }
    Ok(output)
}

/// Identify only native missing-file diagnostics, preserving the logical source.
pub(super) fn missing_file_source(message: &str) -> Option<&str> {
    let (source, reason) = message.split_once(": ")?;
    let prefixes = [
        "[Errno 2] No such file or directory",
        "No such file or directory",
        "[WinError 2] The system cannot find the file specified",
        "[WinError 3] The system cannot find the path specified",
        "The system cannot find the file specified. (os error 2)",
        "The system cannot find the path specified. (os error 3)",
    ];
    (source.starts_with("artifact/source ") && prefixes.iter().any(|p| reason.starts_with(p)))
        .then_some(source)
}

/// Every differing leaf is retained, including absent versus null and array order.
/// Only the previously accepted OS missing-file wording is classified separately.
pub fn compare(expected: &Value, actual: &Value) -> Value {
    fn walk(
        expected: Option<&Value>,
        actual: Option<&Value>,
        path: &str,
        mismatches: &mut Vec<Value>,
        diagnostics: &mut Vec<Value>,
    ) {
        if expected == actual {
            return;
        }
        if let (Some(e), Some(a)) = (expected, actual) {
            if let (Some(e), Some(a)) = (e.as_object(), a.as_object()) {
                for key in e.keys().chain(a.keys()).collect::<BTreeSet<_>>() {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    walk(
                        e.get(key),
                        a.get(key),
                        &format!("{path}/{escaped}"),
                        mismatches,
                        diagnostics,
                    );
                }
                return;
            }
            if let (Some(e), Some(a)) = (e.as_array(), a.as_array()) {
                for i in 0..e.len().max(a.len()) {
                    walk(
                        e.get(i),
                        a.get(i),
                        &format!("{path}/{i}"),
                        mismatches,
                        diagnostics,
                    );
                }
                return;
            }
        }
        let mut diff = json!({"path": path, "expected_present": expected.is_some(),
                              "actual_present": actual.is_some(), "expected": expected, "actual": actual});
        if path.ends_with("/reason") {
            if let (Some(e), Some(a)) = (
                expected.and_then(Value::as_str),
                actual.and_then(Value::as_str),
            ) {
                if missing_file_source(e).is_some()
                    && missing_file_source(e) == missing_file_source(a)
                {
                    diff["classification"] = json!("os-missing-file-wording");
                    diagnostics.push(diff);
                    return;
                }
            }
        }
        mismatches.push(diff);
    }
    let (mut mismatches, mut diagnostics) = (Vec::new(), Vec::new());
    walk(
        Some(expected),
        Some(actual),
        "",
        &mut mismatches,
        &mut diagnostics,
    );
    json!({"mismatches": mismatches, "diagnostic_variations": diagnostics})
}

/// Batch transport for the migration example, deliberately separate from the CLI.
pub fn replay(input: &Value) -> Result<Value> {
    require(
        input["schema"] == "generic-core-differential-input/v1",
        "unknown replay schema",
    )?;
    let cases = input["cases"]
        .as_array()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| error("replay requires nonempty cases"))?;
    let mut ids = BTreeSet::new();
    let mut results = Vec::new();
    let mut count = 0;
    for case in cases {
        let id = case["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| error("missing replay case id"))?;
        require(ids.insert(id), "duplicate replay case id")?;
        let expected = case
            .get("expected_output")
            .ok_or_else(|| error("missing replay expected output"))?;
        let actual = evaluate(case)?;
        let mut result = compare(expected, &actual);
        count += result["mismatches"].as_array().unwrap().len();
        result["id"] = json!(id);
        result["actual"] = actual;
        results.push(result);
    }
    Ok(
        json!({"schema": "generic-core-differential-result/v1", "authoritative": false,
              "case_count": results.len(), "mismatch_count": count, "cases": results}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn differences_preserve_all_fields_and_exact_values() {
        let expected = super::super::parse(
            r#"{"a/b~":[100000000000000000000,2],"missing":null,"state":"pass"}"#,
        )
        .unwrap();
        let actual =
            super::super::parse(r#"{"a/b~":[100000000000000000001,3,4],"state":"fail"}"#).unwrap();
        let result = compare(&expected, &actual);
        let diffs = result["mismatches"].as_array().unwrap();
        assert_eq!(diffs.len(), 5);
        assert_eq!(diffs[0]["path"], "/a~1b~0/0");
        assert_eq!(diffs[3]["actual_present"], false);
        assert_eq!(diffs[3]["expected_present"], true);
        assert!(compare(&expected, &expected)["mismatches"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    #[test]
    fn only_known_missing_file_wording_is_classified() {
        let e = json!({"reason":"artifact/source src/a: No such file or directory: /tmp/a"});
        let a = json!({"reason":"artifact/source src/a: No such file or directory (os error 2)"});
        assert_eq!(
            compare(&e, &a)["diagnostic_variations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        for reason in [
            "[Errno 2] No such file or directory: /tmp/a",
            "[WinError 2] The system cannot find the file specified: C:/a",
            "[WinError 3] The system cannot find the path specified: C:/a",
            "The system cannot find the file specified. (os error 2)",
            "The system cannot find the path specified. (os error 3)",
        ] {
            let actual = json!({"reason": format!("artifact/source src/a: {reason}")});
            let result = compare(&e, &actual);
            assert!(result["mismatches"].as_array().unwrap().is_empty());
            assert_eq!(result["diagnostic_variations"].as_array().unwrap().len(), 1);
        }
        assert_eq!(missing_file_source("no separator"), None);
        assert_eq!(
            missing_file_source("other: No such file or directory"),
            None
        );
        for a in [
            json!({"reason":"artifact/source src/b: No such file or directory"}),
            json!({"reason":"artifact/source src/a: Permission denied"}),
            json!({"reason":"artifact/source src/a: [WinError 5] Access is denied"}),
            json!({"state":"pass"}),
        ] {
            assert!(!compare(&e, &a)["mismatches"].as_array().unwrap().is_empty());
        }
    }
    #[test]
    fn transport_rejects_missing_context_and_empty_batches() {
        assert!(replay(&json!({"schema":"wrong", "cases":[]})).is_err());
        assert!(
            replay(&json!({"schema":"generic-core-differential-input/v1", "cases":[]})).is_err()
        );
        assert!(evaluate(&json!({"policy":{}, "now":"2026-09-08T00:00:00Z", "head":{}})).is_err());
        assert!(evaluate(&json!({"policy":{}, "head":{}})).is_err());
    }
}

#[cfg(test)]
mod acceptance {
    use super::*;
    use std::{fs, process::Command};

    #[test]
    fn retained_corpora_and_consolidated_negative_matrix() {
        let work = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let input_path = work.path().join("input.json");
        let output = Command::new("python3")
            .arg(root.join("../quality/fixtures/generic-core/differential.py"))
            .arg("--prepare")
            .arg(work.path())
            .arg("--output")
            .arg(&input_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let input = super::super::parse(&fs::read_to_string(input_path).unwrap()).unwrap();
        let result = replay(&input).unwrap();
        assert_eq!(result["case_count"], 33);
        assert_eq!(result["mismatch_count"], 0, "{result}");
        // Explicit fail-closed checks in addition to complete golden equivalence.
        let negative = [
            "missing-evidence",
            "stale-context",
            "tampered-artifact",
            "missing-source",
            "duplicate-subject",
            "required-unsupported",
            "malformed-value",
            "unknown-capability",
            "incompatible-series",
            "missing-base",
            "invalid-exception",
            "unknown-producer",
        ];
        for category in negative {
            let cases: Vec<_> = result["cases"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|c| c["id"].as_str().unwrap().ends_with(category))
                .collect();
            assert!(!cases.is_empty(), "missing negative category {category}");
            for case in cases {
                let actual = &case["actual"];
                assert!(
                    actual.get("evaluation_error").is_some()
                        || matches!(
                            actual["policy_result"]["aggregate"]["state"].as_str(),
                            Some("fail" | "blocked" | "measurement_error")
                        ),
                    "negative became favorable: {}",
                    case["id"]
                );
            }
        }
        let breaking = result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "contract-breaking")
            .unwrap();
        assert_eq!(
            breaking["actual"]["project_report"]["aggregate"]["state"],
            "fail"
        );
        // Prove the comparator blocks a counter mutation, with a useful field path.
        let mut changed = result["cases"][0]["actual"].clone();
        changed["policy_result"]["aggregate"]["state"] = json!("unexplained");
        let diff = compare(&result["cases"][0]["actual"], &changed);
        assert_eq!(
            diff["mismatches"][0]["path"],
            "/policy_result/aggregate/state"
        );
        eprintln!("33 retained Rust/Angular/contract cases; 12 negative categories; zero unexplained mismatches");
    }
}
