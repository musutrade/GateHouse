//! Differential tests for only the evidence/project slice of the frozen oracle.
use super::{evidence, json, project, schema, Error, ReasonClass, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, Weak},
};
use tempfile::TempDir;

fn quality() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("quality")
}

// Active tests own the fixture. A static strong owner would never run TempDir's
// destructor when the test process exits (including successful nextest runs).
pub(super) type Reference = (TempDir, Vec<Value>);
static REFERENCE: Mutex<Weak<Reference>> = Mutex::new(Weak::new());

pub(super) fn cached_reference(
    cache: &Mutex<Weak<Reference>>,
    load: fn() -> Reference,
) -> Arc<Reference> {
    let mut weak = cache.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(reference) = weak.upgrade() {
        return reference;
    }
    let reference = Arc::new(load());
    *weak = Arc::downgrade(&reference);
    reference
}

fn reference() -> Arc<Reference> {
    cached_reference(&REFERENCE, load_reference)
}

fn load_reference() -> Reference {
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new("python3")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("quality-core/tests/reference.py"))
        .arg(quality())
        .arg(temp.path())
        .output()
        .expect("Python reference interpreter");
    assert!(
        output.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cases = json::parse(&fs::read_to_string(temp.path().join("cases.json")).unwrap()).unwrap();
    eprintln!("{}", String::from_utf8_lossy(&output.stdout));
    (temp, cases.as_array().unwrap().clone())
}

fn context(case: &Value) -> evidence::ValidationContext<'_> {
    evidence::ValidationContext {
        project: &case["project"],
        expected: &case["expected"],
        source_root: Path::new(case["source_root"].as_str().unwrap()),
        artifact_root: Path::new(case["artifact_root"].as_str().unwrap()),
    }
}

fn assert_outcome(case: &Value, result: Result<Value>) {
    let oracle = &case["oracle"];
    match result {
        Ok(actual) => {
            assert_eq!(
                oracle["accepted"], true,
                "{}: Rust accepted: {oracle}",
                case["name"]
            );
            assert_eq!(actual, oracle["value"], "{}", case["name"]);
        }
        Err(Error { class, message }) => {
            assert_eq!(
                oracle["accepted"], false,
                "{}: Rust rejected: {message}",
                case["name"]
            );
            assert_eq!(
                format!("{class:?}"),
                oracle["reason_class"].as_str().unwrap(),
                "{}",
                case["name"]
            );
            let expected = oracle["reason"].as_str().unwrap();
            // Reference schema validation aggregates errors; Rust returns the
            // first. OS error formatting also differs between the runtimes.
            if expected.starts_with('$') {
                assert!(
                    message.starts_with('$'),
                    "{}: {message} versus {expected}",
                    case["name"]
                );
            } else if super::replay::missing_file_source(expected).is_some() {
                assert!(
                    super::replay::missing_file_source(&message)
                        == super::replay::missing_file_source(expected),
                    "{message}"
                );
            } else {
                assert_eq!(message, expected, "{}", case["name"]);
            }
        }
    }
}

#[test]
fn frozen_evidence_and_integrity_matrix_matches_python() {
    let reference = reference();
    // nextest runs each #[test] in a separate process. Keep all consumers of
    // this full oracle matrix together so it is generated once, with the
    // TempDir owned until every assertion (including byte tampering) finishes.
    project_identity_ownership_and_relationships_match_python(&reference);
    measurement_series_compatibility_matches_python(&reference);
    capability_availability_matches_python(&reference);
    source_and_artifact_bytes_are_verified_at_the_boundary(&reference);
    for case in reference.1.iter().filter(|c| c["kind"] == "evidence") {
        let ctx = context(case);
        let result = evidence::validate_evidence(&case["records"], &ctx).map(|typed| {
            assert_eq!(
                serde_json::to_value(typed).unwrap(),
                case["records"],
                "{}: typed round trip",
                case["name"]
            );
            let bytes = json::canonical(&case["records"]).unwrap();
            json!(super::digest(&bytes))
        });
        assert_outcome(case, result);
    }
}

fn project_identity_ownership_and_relationships_match_python(reference: &Reference) {
    for case in reference.1.iter().filter(|c| c["kind"] == "project") {
        assert_outcome(
            case,
            project::validate_project(&case["project"]).map(|p| serde_json::to_value(p).unwrap()),
        );
    }
}

fn measurement_series_compatibility_matches_python(reference: &Reference) {
    for case in reference.1.iter().filter(|c| c["kind"] == "series") {
        assert_outcome(
            case,
            evidence::require_compatible_series(
                (!case["base"].is_null()).then_some(&case["base"]),
                &case["head"],
            )
            .map(|()| Value::Null),
        );
    }
}

fn capability_availability_matches_python(reference: &Reference) {
    for case in reference.1.iter().filter(|c| c["kind"] == "requirements") {
        assert_outcome(
            case,
            evidence::evaluate_requirements(
                &case["records"],
                &case["requirements"],
                &context(case),
            ),
        );
    }
}

#[test]
fn embedded_contracts_match_frozen_reference() {
    for (name, embedded) in [
        ("project-model", &*schema::PROJECT),
        ("harness-evidence", &*schema::EVIDENCE),
        ("capability-requirements", &*schema::REQUIREMENTS),
        ("policy", &*schema::POLICY),
        ("policy-exceptions", &*schema::EXCEPTIONS),
        ("subject-mappings", &*schema::MAPPINGS),
    ] {
        let reference = json::parse(
            &fs::read_to_string(quality().join(format!("schema/{name}.schema.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(*embedded, reference, "{name}");
    }
}

#[test]
fn json_boundary_rejects_duplicate_keys_and_malformed_values() {
    assert!(json::parse(&format!("{}0{}", "[".repeat(129), "]".repeat(129))).is_err());
    assert_eq!(json::canonical(&json::parse("-0").unwrap()).unwrap(), b"0");
    for text in [
        r#"{"a":1,"a":2}"#,
        r#"[{"a":{"x":1,"\u0078":2}}]"#,
        "NaN",
        "Infinity",
        "-Infinity",
        "[1,]",
        "{} {}",
        r#""\ud800""#,
    ] {
        assert_eq!(
            json::parse(text).unwrap_err().class,
            ReasonClass::MeasurementError,
            "{text}"
        );
    }
    for text in ["1.0", "1e2", "-0.0", r#"{"value":"\u0000"}"#, r#"{"\n":1}"#] {
        assert!(
            json::canonical(&json::parse(text).unwrap()).is_err(),
            "{text}"
        );
    }
    let text = r#"{"z":10000000000000000000000000000000000000001,"é":"😀/é","a":-123456789012345678901234567890}"#;
    assert_eq!(
        String::from_utf8(json::canonical(&json::parse(text).unwrap()).unwrap()).unwrap(),
        r#"{"a":-123456789012345678901234567890,"z":10000000000000000000000000000000000000001,"é":"😀/é"}"#
    );
}

fn source_and_artifact_bytes_are_verified_at_the_boundary(reference: &Reference) {
    let case = reference
        .1
        .iter()
        .find(|c| c["name"] == "polyglot")
        .unwrap();
    assert_eq!(
        super::digest(&evidence::canonical_serialize(&case["records"], &context(case)).unwrap()),
        case["oracle"]["value"]
    );
    for source in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let mut ctx = context(case);
        let record = &case["records"][0];
        let (original, relative) = if source {
            (ctx.source_root, record["source"]["path"].as_str().unwrap())
        } else {
            (
                ctx.artifact_root,
                record["artifacts"][0]["path"].as_str().unwrap(),
            )
        };
        let target = temp.path().join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        if source {
            ctx.source_root = temp.path();
        } else {
            ctx.artifact_root = temp.path();
        }
        let records = json!([record]);
        assert!(super::replay::missing_file_source(
            &evidence::validate_evidence(&records, &ctx)
                .unwrap_err()
                .message
        )
        .is_some());
        fs::create_dir(&target).unwrap();
        assert_eq!(
            evidence::validate_evidence(&records, &ctx)
                .unwrap_err()
                .message,
            "artifact/source is not a regular file"
        );
        fs::remove_dir(&target).unwrap();
        fs::write(&target, b"tampered").unwrap();
        assert_eq!(
            evidence::validate_evidence(&records, &ctx)
                .unwrap_err()
                .message,
            "artifact/source digest mismatch"
        );
        fs::copy(original.join(relative), &target).unwrap();
        evidence::validate_evidence(&records, &ctx).unwrap();
        #[cfg(unix)]
        {
            fs::remove_file(&target).unwrap();
            std::os::unix::fs::symlink(original.join(relative), &target).unwrap();
            assert_eq!(
                evidence::validate_evidence(&records, &ctx)
                    .unwrap_err()
                    .message,
                "path escapes declared root"
            );
        }
    }
}

#[test]
fn typed_values_preserve_arbitrary_precision() {
    for text in ["9".repeat(100), format!("1{}", "0".repeat(100))] {
        let huge: Value = serde_json::from_str(&text).unwrap();
        for raw in [
            json!({"type": "ratio", "covered": huge, "total": huge}),
            json!({"type": "count", "value": huge}),
            json!({"type": "duration", "value": huge, "unit": "ns"}),
            json!({"type": "size", "value": huge, "unit": "bytes"}),
            json!({"type": "rational", "numerator": huge, "denominator": huge}),
            json!({"type": "decimal", "value": "123.456"}),
            json!({"type": "boolean", "value": true}),
        ] {
            let typed: super::model::MetricValue = json::decode(&raw).unwrap();
            assert_eq!(serde_json::to_value(typed).unwrap(), raw);
        }
    }
}

#[test]
fn oracle_cache_releases_files_after_last_owner_and_unwind() {
    fn fixture() -> Reference {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("fixture"), "owned bytes").unwrap();
        (temp, Vec::new())
    }
    let cache = Mutex::new(Weak::new());
    let first = cached_reference(&cache, fixture);
    let path = first.0.path().to_owned();
    let second = cached_reference(&cache, fixture);
    assert!(Arc::ptr_eq(&first, &second));
    drop(first);
    assert!(path.join("fixture").is_file());
    drop(second);
    assert!(!path.exists());
    assert!(cache.lock().unwrap().upgrade().is_none());

    let next = cached_reference(&cache, fixture);
    let next_path = next.0.path().to_owned();
    let result = std::panic::catch_unwind(move || {
        let _owner = next;
        panic!("simulate failed test");
    });
    assert!(result.is_err());
    assert!(!next_path.exists());
    assert!(cache.lock().unwrap().upgrade().is_none());
}
