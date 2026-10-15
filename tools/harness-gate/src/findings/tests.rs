use super::*;

fn core() -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":"1", "evidence_complete":true,
        "status":"FAIL", "passed":false, "invocation_id":"run-1",
        "source_identity":format!("git-tree:{}", "b".repeat(40))}))
    .unwrap()
}

fn bundle() -> Bundle {
    serde_json::from_value(json!({"schema_version":VERSION,
        "context":{"repository":"owner/repo", "commit":"a".repeat(40), "tree":"b".repeat(40), "series":"test/1"},
        "report_sha256":digest(&core()), "sources":[{"format":"native/1", "complete":true,
        "measurement_id":"measurement-1", "document":{"schema_version":"native/1", "tool":"lint", "tool_version":"1",
        "findings":[{"rule":"R1", "anchor":"symbol-1", "severity":"error", "message":"password=private-value",
        "locations":[{"path":"src/main.rs", "line":3, "end_line":3}], "fixes":null,
        "custom":{"token":"private-token", "score":7}}]}}]})).unwrap()
}

fn normalized() -> Report {
    normalize(bundle(), &core()).unwrap()
}

#[test]
fn provenance_redaction_and_sarif_roundtrip() {
    let report = normalized();
    assert_eq!(report.core_status, "FAIL");
    assert_eq!(report.findings[0].message, "[REDACTED]");
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("private-value"));
    assert!(!serialized.contains("private-token"));
    assert_eq!(
        report.sources[0].document["findings"][0]["custom"]["score"],
        7
    );
    assert_eq!(report.sources[0].measurement_id, "measurement-1");
    let sarif = sarif::export(&report);
    let findings = sarif::import(&sarif, 0).unwrap();
    assert_eq!(findings[0].id, report.findings[0].id);
    assert_eq!(findings[0].tool_version, "1");
    assert_eq!(findings[0].locations, report.findings[0].locations);
    load(&serde_json::to_vec(&report).unwrap()).unwrap();
}

#[test]
fn history_preserves_moves_duplicates_resolution_and_reopening() {
    let before = normalized();
    let original_id = before.findings[0].id.clone();
    let mut next = bundle();
    next.context.commit = "c".repeat(40);
    next.sources[0].document["findings"][0]["locations"][0] =
        json!({"path":"lib/renamed.rs", "line":90, "end_line":91});
    let mut after = normalize(next, &core()).unwrap();
    after.findings.push(after.findings[0].clone());
    let after = history::compare(after, before, &"0".repeat(64)).unwrap();
    assert_eq!(after.history.len(), 1);
    assert_eq!(after.history[0].state, "persistent");
    assert_eq!(after.history[0].finding.id, original_id);
    assert_eq!(after.history[0].first_seen, "a".repeat(40));
    let mut empty = bundle();
    empty.context.commit = "d".repeat(40);
    empty.sources[0].document["findings"] = json!([]);
    let resolved =
        history::compare(normalize(empty, &core()).unwrap(), after, &"1".repeat(64)).unwrap();
    assert_eq!(resolved.history[0].state, "resolved");
    let mut back = bundle();
    back.context.commit = "e".repeat(40);
    let reopened =
        history::compare(normalize(back, &core()).unwrap(), resolved, &"2".repeat(64)).unwrap();
    assert_eq!(reopened.history[0].state, "reopened");
    assert_eq!(reopened.history[0].first_seen, "a".repeat(40));
    assert_eq!(reopened.previous_sha256, Some("2".repeat(64)));
}

#[test]
fn rejects_incompatible_incomplete_and_forged_inputs() {
    let mut cases = Vec::new();
    let mut b = bundle();
    b.schema_version = "future".into();
    cases.push(b);
    let mut b = bundle();
    b.context.commit = "main".into();
    cases.push(b);
    let mut b = bundle();
    b.context.repository = "../repo".into();
    cases.push(b);
    let mut b = bundle();
    b.report_sha256 = "0".repeat(64);
    cases.push(b);
    let mut b = bundle();
    b.sources[0].complete = false;
    cases.push(b);
    let mut b = bundle();
    b.sources[0].format = "unknown".into();
    cases.push(b);
    let mut b = bundle();
    b.sources[0].measurement_id.clear();
    cases.push(b);
    let mut b = bundle();
    b.sources.push(b.sources[0].clone());
    cases.push(b);
    let mut b = bundle();
    b.sources[0].document["findings"][0]["severity"] = json!("critical");
    cases.push(b);
    let mut b = bundle();
    b.sources[0].document["findings"][0]["locations"] = json!([]);
    cases.push(b);
    let mut b = bundle();
    b.sources[0].document["tool_version"] = Value::Null;
    cases.push(b);
    for b in cases {
        assert!(normalize(b, &core()).is_err());
    }
    for field in [
        "evidence_complete",
        "status",
        "source_identity",
        "invocation_id",
    ] {
        let mut value: Value = parse(&core()).unwrap();
        value[field] = Value::Null;
        let bytes = serde_json::to_vec(&value).unwrap();
        let mut b = bundle();
        b.report_sha256 = digest(&bytes);
        assert!(normalize(b, &bytes).is_err(), "{field}");
    }
    assert!(parse(br#"{"a":1,"a":2}"#).is_err());
    let mut report = normalized();
    report.findings[0].id = "forged".into();
    assert!(load(&serde_json::to_vec(&report).unwrap()).is_err());
}

#[test]
fn paths_are_confined_and_lines_validated() {
    for path in [
        "../escape",
        "/root",
        "C:/root",
        "a\\b",
        "a//b",
        "a/./b",
        "a/%2e%2e/b",
        "a?b",
        "a#b",
        "a\nb",
        "",
    ] {
        assert!(
            validate_location(&Location {
                path: path.into(),
                line: 1,
                end_line: 1
            })
            .is_err(),
            "{path}"
        );
    }
    assert!(validate_location(&Location {
        path: "src/a b.rs".into(),
        line: 1,
        end_line: 2
    })
    .is_ok());
    assert!(validate_location(&Location {
        path: "a.rs".into(),
        line: 0,
        end_line: 0
    })
    .is_err());
    assert!(validate_location(&Location {
        path: "a.rs".into(),
        line: 2,
        end_line: 1
    })
    .is_err());
}

#[test]
fn history_refuses_unstable_identity_and_source_coverage_changes() {
    let before = normalized();
    let mut current = bundle();
    current.sources[0].document["findings"][0]["anchor"] = Value::Null;
    let current = normalize(current, &core()).unwrap();
    assert!(!current.diagnostics.is_empty());
    assert!(history::compare(current, before.clone(), "pin").is_err());
    let mut current = normalized();
    current.context.series = "new-series".into();
    assert!(history::compare(current, before.clone(), "pin").is_err());
    let mut current = normalized();
    current.sources[0].document["tool_version"] = json!("2");
    assert!(history::compare(current, before.clone(), "pin").is_err());
    let mut current = normalized();
    current.history.clear();
    assert!(history::compare(current, before, "pin").is_err());
}

#[test]
fn sarif_diagnoses_unsupported_results_and_preserves_extensions() {
    let source = sarif::export(&normalized());
    let mut cases = Vec::new();
    let mut v = source.clone();
    v["version"] = json!("2.0.0");
    cases.push(v);
    let mut v = source.clone();
    v["runs"] = json!([]);
    cases.push(v);
    let mut v = source.clone();
    v["runs"][0]["results"] = Value::Null;
    cases.push(v);
    let mut v = source.clone();
    v["runs"][0]["invocations"][0]["executionSuccessful"] = json!(false);
    cases.push(v);
    let mut v = source.clone();
    v["runs"][0]["results"][0]["kind"] = json!("pass");
    cases.push(v);
    let mut v = source.clone();
    v["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
        ["uriBaseId"] = json!("ROOT");
    cases.push(v);
    let mut v = source.clone();
    v["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]["endLine"] = json!(-1);
    cases.push(v);
    let mut v = source.clone();
    v["runs"][0]["results"][0]["message"] = json!({"markdown":"only"});
    cases.push(v);
    for v in cases {
        assert!(sarif::import(&v, 0).is_err());
    }
    let mut b = bundle();
    b.sources[0].format = "sarif/2.1.0".into();
    b.sources[0].document = source;
    b.sources[0].document["extension"] = json!({"number":17});
    let report = normalize(b, &core()).unwrap();
    assert_eq!(report.sources[0].document["extension"]["number"], 17);
    assert_eq!(report.findings[0].rule, "R1");
    assert_eq!(report.findings[0].pointer, "/runs/0/results/0");
}

#[test]
fn load_rejects_history_and_provenance_mutation() {
    let original = serde_json::to_value(normalized()).unwrap();
    let mutations: Vec<(&str, Value)> = vec![
        ("/schema_version", json!("other")),
        ("/context/series", json!("")),
        ("/core_status", json!("UNKNOWN")),
        ("/invocation_id", json!("")),
        ("/source_identity", json!("working-tree:head")),
        ("/report_sha256", json!("short")),
        ("/sources", json!([])),
        ("/sources/0/complete", json!(false)),
        ("/sources/0/measurement_id", json!("")),
        ("/sources/0/format", json!("other")),
        (
            "/sources/0/document/findings/0/message",
            json!("password=exposed"),
        ),
        ("/findings/0/message", json!("forged message")),
        ("/history/0/finding/id", json!("forged")),
        ("/history/0/first_seen", json!("main")),
        ("/history/0/state", json!("hidden")),
        ("/history/0/last_seen", json!("c".repeat(40))),
        ("/history/0/evidence/complete", json!(false)),
        ("/previous_sha256", json!("short")),
    ];
    for (pointer, replacement) in mutations {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            load(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{pointer}"
        );
    }
    let mut report = normalized();
    report.history.push(report.history[0].clone());
    assert!(load(&serde_json::to_vec(&report).unwrap()).is_err());
    let mut report = normalized();
    report.history.clear();
    assert!(load(&serde_json::to_vec(&report).unwrap()).is_err());
    let mut report = normalized();
    report.sources.push(report.sources[0].clone());
    assert!(load(&serde_json::to_vec(&report).unwrap()).is_err());
}

#[test]
fn sarif_defaults_unstable_diagnostics_and_fix_suggestions_roundtrip() {
    let mut source = sarif::export(&normalized());
    let result = &mut source["runs"][0]["results"][0];
    result
        .as_object_mut()
        .unwrap()
        .remove("partialFingerprints");
    result.as_object_mut().unwrap().remove("level");
    result["fixes"] = json!([{"description":{"text":"Review and close the resource"}}]);
    let driver = &mut source["runs"][0]["tool"]["driver"];
    driver["rules"] = json!([{"id":"R1", "defaultConfiguration":{"level":"note"}}]);
    let mut input = bundle();
    input.sources[0].format = "sarif/2.1.0".into();
    input.sources[0].document = source;
    let report = normalize(input, &core()).unwrap();
    assert!(!report.findings[0].stable);
    assert_eq!(report.findings[0].severity, "note");
    let roundtrip = sarif::import(&sarif::export(&report), 0).unwrap();
    assert_eq!(roundtrip[0].fixes, report.findings[0].fixes);
    assert!(!roundtrip[0].stable);
}

#[test]
fn input_output_bounds_and_no_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("large.json");
    fs::File::create(&input)
        .unwrap()
        .set_len(MAX_BYTES + 1)
        .unwrap();
    assert!(read(&input).is_err());
    let output = temp.path().join("result.json");
    write(&output, &json!({"status":"retained"})).unwrap();
    assert!(write(&output, &json!({"status":"overwritten"})).is_err());
    assert_eq!(
        parse(&read(&output).unwrap()).unwrap()["status"],
        "retained"
    );
    let mut input = bundle();
    input.sources.clear();
    assert!(normalize(input, &core()).is_err());
    let mut input = bundle();
    input.sources[0].document["findings"] = json!([]);
    let empty = normalize(input, &core()).unwrap();
    assert!(empty.findings.is_empty());
    assert!(sarif::import(&sarif::export(&empty), 0).unwrap().is_empty());
}

#[test]
fn sarif_uri_paths_roundtrip_without_traversal_or_double_decoding() {
    let mut input = bundle();
    input.sources[0].document["findings"][0]["locations"][0]["path"] = json!("src/空 格.rs");
    let report = normalize(input, &core()).unwrap();
    let exported = sarif::export(&report);
    assert_eq!(
        exported["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"],
        "src/%E7%A9%BA%20%E6%A0%BC.rs"
    );
    assert_eq!(
        sarif::import(&exported, 0).unwrap()[0].locations[0].path,
        "src/空 格.rs"
    );
    for path in [
        "%2e%2e/a.rs",
        "a/%252e%252e/b.rs",
        "a%2fb.rs",
        "a%5cb.rs",
        "a/%",
        "a/%QQ",
        "a/%FF",
        "file:///tmp/a.rs",
    ] {
        let mut exported = exported.clone();
        exported["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
            ["uri"] = json!(path);
        assert!(sarif::import(&exported, 0).is_err(), "{path}");
    }
}

#[test]
fn independent_tool_observations_share_only_explicit_canonical_identity() {
    let mut input = bundle();
    let mut document = sarif::export(&normalized());
    document["runs"][0]["tool"]["driver"]["name"] = json!("another-checker");
    input.sources.push(Source {
        format: "sarif/2.1.0".into(),
        measurement_id: "measurement-2".into(),
        complete: true,
        document,
    });
    let report = normalize(input, &core()).unwrap();
    assert_eq!(report.findings.len(), 2);
    assert_eq!(report.findings[0].id, report.findings[1].id);
    assert_ne!(report.findings[0].tool, report.findings[1].tool);
    assert_eq!(report.history.len(), 1);
    assert_eq!(report.sources.len(), 2);
    load(&serde_json::to_vec(&report).unwrap()).unwrap();
}

#[test]
fn historical_snapshots_do_not_copy_the_full_result_set_per_finding() {
    let mut input = bundle();
    let finding = input.sources[0].document["findings"][0].clone();
    input.sources[0].document["findings"] = Value::Array(
        (0..500)
            .map(|i| {
                let mut value = finding.clone();
                value["anchor"] = json!(format!("symbol-{i}"));
                value
            })
            .collect(),
    );
    let report = normalize(input, &core()).unwrap();
    assert_eq!(report.history.len(), 500);
    for record in &report.history {
        assert_eq!(
            record.evidence.document["findings"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    let bytes = serde_json::to_vec(&report).unwrap();
    assert!(
        bytes.len() < 2_000_000,
        "quadratic history growth: {} bytes",
        bytes.len()
    );
    load(&bytes).unwrap();
}
