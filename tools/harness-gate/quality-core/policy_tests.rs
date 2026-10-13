//! Differential tasks 3–4 tests. The Python process is a test oracle, never runtime.
use super::tests::{cached_reference, Reference};
use super::{evidence::ValidationContext, json, policy, project_report, ratchet, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::{Arc, Mutex, Weak},
};

static REFERENCE: Mutex<Weak<Reference>> = Mutex::new(Weak::new());

fn reference() -> Arc<Reference> {
    cached_reference(&REFERENCE, load_reference)
}

fn load_reference() -> Reference {
    let temp = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("quality-core/tests/policy_reference.py"))
        .arg(root.parent().unwrap().join("quality"))
        .arg(temp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    let cases = json::parse(&fs::read_to_string(temp.path().join("cases.json")).unwrap()).unwrap();
    (temp, cases.as_array().unwrap().clone())
}

fn context(value: &Value) -> ValidationContext<'_> {
    ValidationContext {
        project: &value["project"],
        expected: &value["expected"],
        source_root: Path::new(value["source_root"].as_str().unwrap()),
        artifact_root: Path::new(value["artifact_root"].as_str().unwrap()),
    }
}

fn run(case: &Value) -> Result<Value> {
    let a = &case["args"];
    let k = &case["kwargs"];
    match case["kind"].as_str().unwrap() {
        "evaluate" => {
            let ctx = context(k);
            let base = k.get("base_context").filter(|v| !v.is_null()).map(context);
            policy::evaluate(
                &a[0],
                &a[1],
                &ctx,
                &policy::EvaluationOptions {
                    selection: k.get("selection"),
                    base_records: k.get("base_records").filter(|v| !v.is_null()),
                    base_context: base.as_ref(),
                    mappings: k.get("mappings").filter(|v| !v.is_null()),
                    exceptions: k.get("exceptions"),
                    now: k["now"].as_str(),
                },
            )
        }
        "report" => project_report::report(&a[0], &a[1], &a[2]),
        "decision" => ratchet::decision(&a[0], &a[1], Some(&a[2])).map(|v| json!(v)),
        "compare" => policy::compare(&a[0], a[1].as_str().unwrap(), &a[2]).map(Value::Bool),
        "aggregate" => policy::aggregate(
            &a[0],
            &serde_json::from_value::<Vec<policy::GateResult>>(a[1].clone()).unwrap(),
        ),
        "validate_policy" => policy::validate_policy(&a[0], &a[1]).map(|_| a[0].clone()),
        "_select" => policy::select(&a[0], &a[1], &a[2], &a[3]).map(|v| json!(v)),
        "review_exceptions" => Ok(ratchet::review_exceptions(
            &a[0],
            &a[1],
            &a[2],
            k["now"].as_str(),
        )),
        other => panic!("unknown test kind {other}"),
    }
}

fn difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    // GH-147 already classified runtime-specific missing-file diagnostics.
    // Compare the path and failure category; retain every semantic field exactly.
    if path.ends_with(".reason") {
        if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
            if super::replay::missing_file_source(a).is_some()
                && super::replay::missing_file_source(a) == super::replay::missing_file_source(b)
            {
                return None;
            }
        }
    }
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        if a.keys().eq(b.keys()) {
            return a
                .iter()
                .find_map(|(key, value)| difference(value, &b[key], &format!("{path}.{key}")));
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            return a
                .iter()
                .zip(b)
                .enumerate()
                .find_map(|(i, (a, b))| difference(a, b, &format!("{path}[{i}]")));
        }
    }
    Some(format!("{path}: Rust {a} versus Python {b}"))
}

#[test]
fn frozen_policy_and_reference_acceptance_match_python() {
    let reference = reference();
    // This focused acceptance case consumes the same generated oracle. Keep
    // it in this process under nextest instead of generating the matrix twice.
    green_local_gates_and_breaking_contract_block_project_with_provenance(&reference);
    let mut mismatches = Vec::new();
    for case in &reference.1 {
        let actual = match run(case) {
            Ok(value) => {
                if let Some(expected) = case.get("project_report") {
                    let report = project_report::report(
                        &value,
                        &case["kwargs"]["project"],
                        &case["args"][0],
                    )
                    .unwrap();
                    if let Some(diff) = difference(&report, expected, "$.project_report") {
                        mismatches.push(format!("{}: {diff}", case["name"]));
                    }
                }
                json!({"accepted":true,"value":value})
            }
            Err(e) => {
                json!({"accepted":false,"reason_class":format!("{:?}",e.class),"reason":e.message})
            }
        };
        if let Some(diff) = difference(&actual, &case["oracle"], "$") {
            mismatches.push(format!("{}: {diff}", case["name"]));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

fn green_local_gates_and_breaking_contract_block_project_with_provenance(reference: &Reference) {
    let case = reference
        .1
        .iter()
        .find(|c| c["name"] == "contract-breaking")
        .unwrap();
    let result = run(case).unwrap();
    let report =
        project_report::report(&result, &case["kwargs"]["project"], &case["args"][0]).unwrap();
    assert_eq!(report["aggregate"]["state"], "fail");
    for component in ["api", "frontend"] {
        assert_eq!(report["components"][component]["local"]["state"], "pass");
        assert_eq!(
            report["components"][component]["cross_component"]["state"],
            "fail"
        );
        assert_eq!(
            report["components"][component]["aggregate"]["state"],
            "fail"
        );
    }
    assert_eq!(report["gates"].as_object().unwrap().len(), 5);
    assert_eq!(report["aggregate"]["blockers"].as_array().unwrap().len(), 3);
    let gate = report["gates"]
        .as_object()
        .unwrap()
        .values()
        .find(|g| g["policy"] == "contract.breaking_changes")
        .unwrap();
    assert_eq!(gate["record"]["relationship"]["producer"], "api");
    assert_eq!(gate["record"]["relationship"]["consumer"], "frontend");
    assert!(!gate["record"]["evidence_links"]["head"]["artifacts"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(difference(&report, &case["project_report"], "$").is_none());
}

fn scope_policy(scope: Value) -> Value {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/workflow/compiler/direct.json")).unwrap();
    let mut policy = fixture["policy"].clone();
    policy["rules"][0]["scope"] = scope;
    if policy["rules"][0]["scope"]["kind"] == "relationship" {
        policy["rules"][0]["metric"] = json!("contract.compatible");
        policy["rules"][0]["operator"] = json!("eq");
        policy["rules"][0]["limit"] = json!({"type":"boolean", "value":true});
    }
    policy
}

#[test]
fn scope_documents_validate_every_supported_kind_and_reject_invalid_fields() {
    for scope in [
        json!({"kind":"project"}),
        json!({"kind":"component", "component":"app"}),
        json!({"kind":"boundary", "component":"app", "boundary":"source"}),
        json!({"kind":"changed_subject"}),
        json!({"kind":"critical_subject"}),
        json!({"kind":"relationship", "relationship":"api.contract-1"}),
        json!({"kind":"subject", "subject":"module-alias"}),
    ] {
        policy::validate_policy_document(&scope_policy(scope.clone())).unwrap();
        for field in scope.as_object().unwrap().keys() {
            let mut missing = scope.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(policy::validate_policy_document(&scope_policy(missing)).is_err());
            for value in [Value::Null, json!(false), json!(1), json!(""), json!([])] {
                let mut invalid = scope.clone();
                invalid[field] = value;
                assert!(policy::validate_policy_document(&scope_policy(invalid)).is_err());
            }
        }
        let mut extra = scope.clone();
        extra["unexpected"] = json!("value");
        assert!(policy::validate_policy_document(&scope_policy(extra)).is_err());
    }
    for scope in [
        json!({"kind":"unknown"}),
        json!({"kind":"project", "component":"app"}),
        json!({"kind":"project", "boundary":"source"}),
        json!({"kind":"relationship", "relationship":"UPPER"}),
        json!({"kind":"relationship", "relationship":"api\n"}),
        Value::Null,
        json!([]),
    ] {
        assert!(policy::validate_policy_document(&scope_policy(scope)).is_err());
    }
}

#[test]
fn resolved_scope_validation_preserves_alias_compilation_and_canonical_selection() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/workflow/compiler/direct.json")).unwrap();
    let project = &fixture["project"];
    let canonical = &project["subjects"][0]["id"];
    let alias = scope_policy(json!({"kind":"subject", "subject":"module-alias"}));
    policy::validate_policy_document(&alias).unwrap();
    assert_eq!(
        policy::validate_policy(&alias, project)
            .unwrap_err()
            .message,
        "unknown policy subject"
    );
    let resolved = scope_policy(json!({"kind":"subject", "subject":canonical}));
    policy::validate_policy(&resolved, project).unwrap();
    for scope in [
        json!({"kind":"project"}),
        json!({"kind":"component", "component":"app"}),
        json!({"kind":"boundary", "component":"app", "boundary":"source"}),
        json!({"kind":"subject", "subject":canonical}),
        json!({"kind":"changed_subject"}),
        json!({"kind":"critical_subject"}),
    ] {
        policy::validate_policy(&scope_policy(scope.clone()), project).unwrap();
        let selection = json!({"changed_subject":[canonical], "critical_subject":[canonical]});
        assert_eq!(
            policy::select(&scope, project, &selection, &json!("default"))
                .unwrap()
                .len(),
            1
        );
    }
    for scope in [
        json!({"kind":"unknown"}),
        json!({"kind":"project", "component":"app"}),
        json!({"kind":"subject", "subject":"module-alias"}),
    ] {
        assert!(policy::select(&scope, project, &Value::Null, &json!("default")).is_err());
    }
    let newline = json!({"kind":"subject", "subject":format!("{}\n", canonical.as_str().unwrap())});
    assert_eq!(
        policy::select(&newline, project, &Value::Null, &json!("default"))
            .unwrap_err()
            .message,
        "invalid character in evidence string"
    );
}

#[test]
fn duplicate_remediation_classes_are_rejected_with_field_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(root.join("../quality/fixtures/policy/policy.json")).unwrap();
    let mut document = json::parse(&text).unwrap();
    policy::validate_policy_document(&document).expect("fixture policy is valid");
    let class = document["rules"][0]["remediation_classes"][0].clone();
    document["rules"][0]["remediation_classes"] = json!([class.clone(), class]);
    let error = policy::validate_policy_document(&document)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("$.rules[0].remediation_classes: array violates uniqueItems"),
        "{error}"
    );
}
