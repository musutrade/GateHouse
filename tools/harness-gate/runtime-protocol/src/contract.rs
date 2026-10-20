//! Public data-contract tests for `gate-runtime/v1`.
//!
//! These exercise only the implemented protocol surface. They do not claim
//! transport, dispatch counts, persisted intent/unknown state, artifact
//! verification or real engine behavior.

use super::*;
use serde_json::{json, Value};

const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn binding_value() -> Value {
    json!({
        "protocol": "gate-runtime/v1",
        "adapter_sha256": ID_A,
        "adapter_version": "1.0.0",
        "invocation": "inv-1",
        "operation_key": "op-1",
        "generation": "gen-1",
        "namespace": {
            "engine": "docker",
            "endpoint": "unix:///run/docker.sock",
            "context": "ctx-1",
            "principal": "uid:1000",
            "rootless": true,
            "scope": "scope-1"
        }
    })
}

fn request_value(operation: Value) -> Value {
    json!({"binding": binding_value(), "operation": operation})
}

fn response_value(binding: &Value, operation: Value, outcome: Value) -> Value {
    json!({"binding": binding.clone(), "operation": operation, "outcome": outcome})
}

fn inspect_op(id: &str) -> Value {
    json!({"inspect": {"object_id": id}})
}

fn remove_op(id: &str) -> Value {
    json!({"remove": {"object_id": id}})
}

fn present_outcome(id: &str) -> Value {
    json!({"present": {"object": {
        "object_id": id,
        "labels": [{"key": "harness-gate.owner", "value": "harness-gate"}]
    }}})
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn request(value: &Value) -> Request {
    parse_request(&bytes(value)).expect("valid request fixture")
}

fn response(value: &Value, expected: &Request) -> Response {
    parse_response(&bytes(value), expected).expect("valid response fixture")
}

fn reject_request(value: &Value) {
    assert!(parse_request(&bytes(value)).is_err(), "accepted {value}");
}

fn reject_response(value: &Value, expected: &Request) {
    assert!(
        parse_response(&bytes(value), expected).is_err(),
        "accepted {value}"
    );
}

fn raw_request(operation: &str) -> String {
    format!(
        r#"{{"binding":{},"operation":{operation}}}"#,
        serde_json::to_string(&binding_value()).unwrap()
    )
}

#[test]
fn duplicate_and_unknown_fields_are_rejected() {
    let binding = serde_json::to_string(&binding_value()).unwrap();
    let op = format!(r#"{{"inspect":{{"object_id":"{ID_A}"}}}}"#);
    for text in [
        format!(r#"{{"binding":{binding},"binding":{binding},"operation":{op}}}"#),
        format!(r#"{{"binding":{binding},"operation":{op},"extra":1}}"#),
        format!(
            r#"{{"binding":{},"operation":{op}}}"#,
            binding.replace(r#""rootless":true"#, r#""rootless":true,"extra":1"#)
        ),
        raw_request(&format!(
            r#"{{"inspect":{{"object_id":"{ID_A}","extra":1}}}}"#
        )),
        raw_request(&format!(
            r#"{{"inspect":{{"object_id":"{ID_A}"}},"extra":1}}"#
        )),
    ] {
        assert!(
            parse_request(text.as_bytes()).is_err(),
            "accepted request duplicate/unknown fields: {text}"
        );
    }
    let request = request(&request_value(inspect_op(ID_A)));
    let good_binding = binding_value();
    for outcome in [
        json!({"present": {"object": {
            "object_id": ID_A,
            "labels": [{"key": "k", "value": "v", "extra": 1}]
        }}}),
        json!({"present": {"object": {
            "object_id": ID_A,
            "labels": [{"key": "k", "value": "v"}],
            "extra": 1
        }}}),
        json!({"present": {"object": {
            "object_id": ID_A,
            "labels": [{"key": "k", "value": "v"}]
        }, "extra": 1}}),
    ] {
        reject_response(
            &response_value(&good_binding, inspect_op(ID_A), outcome),
            &request,
        );
    }
    let binding_text = serde_json::to_string(&good_binding).unwrap();
    let duplicate_label = format!(
        r#"{{"binding":{binding_text},"operation":{{"inspect":{{"object_id":"{ID_A}"}}}},"outcome":{{"present":{{"object":{{"object_id":"{ID_A}","labels":[{{"key":"k","key":"k2","value":"v"}}]}}}}}}}}"#
    );
    assert!(
        parse_response(duplicate_label.as_bytes(), &request).is_err(),
        "accepted duplicate label field"
    );
}

#[test]
fn multiple_or_duplicate_operation_and_outcome_variants_are_rejected() {
    for text in [
        raw_request(&format!(
            r#"{{"inspect":{{"object_id":"{ID_A}"}},"remove":{{"object_id":"{ID_A}"}}}}"#
        )),
        raw_request(&format!(
            r#"{{"inspect":{{"object_id":"{ID_A}"}},"inspect":{{"object_id":"{ID_A}"}}}}"#
        )),
    ] {
        assert!(parse_request(text.as_bytes()).is_err(), "accepted {text}");
    }
    let request = request(&request_value(inspect_op(ID_A)));
    let binding = serde_json::to_string(&binding_value()).unwrap();
    for text in [
        format!(
            r#"{{"binding":{binding},"operation":{{"inspect":{{"object_id":"{ID_A}"}}}},"outcome":{{"absent":null,"unknown":null}}}}"#
        ),
        format!(
            r#"{{"binding":{binding},"operation":{{"inspect":{{"object_id":"{ID_A}"}}}},"outcome":{{"absent":null,"absent":null}}}}"#
        ),
    ] {
        assert!(
            parse_response(text.as_bytes(), &request).is_err(),
            "accepted {text}"
        );
    }
}

#[test]
fn trailing_json_and_non_object_messages_are_rejected() {
    let mut valid = raw_request(&format!(r#"{{"inspect":{{"object_id":"{ID_A}"}}}}"#));
    assert!(parse_request(valid.as_bytes()).is_ok());
    valid.push_str(" \n\t ");
    assert!(parse_request(valid.as_bytes()).is_ok());
    for suffix in [r#"{}"#, "1", "[", r#""x""#] {
        let text = format!(
            "{}{suffix}",
            raw_request(&format!(r#"{{"inspect":{{"object_id":"{ID_A}"}}}}"#))
        );
        assert!(
            parse_request(text.as_bytes()).is_err(),
            "accepted trailing {suffix}"
        );
    }
    for text in ["", "null", "[]", "1", r#""request""#, "{}", "true"] {
        assert!(
            parse_request(text.as_bytes()).is_err(),
            "accepted non-object {text}"
        );
    }
    let mut invalid_utf8 =
        raw_request(&format!(r#"{{"inspect":{{"object_id":"{ID_A}"}}}}"#)).into_bytes();
    invalid_utf8.push(0xff);
    assert!(parse_request(&invalid_utf8).is_err());
}

#[test]
fn wrong_types_are_rejected() {
    let cases = [
        json!({"binding": binding_value(), "operation": {"inspect": {"object_id": 1}}}),
        json!({"binding": binding_value(), "operation": {"inspect": {"object_id": ID_A}}, "extra": true}),
    ];
    for case in cases {
        reject_request(&case);
    }
    let mut cases = Vec::new();
    for (field, value) in [
        ("protocol", json!(1)),
        ("adapter_sha256", json!(true)),
        ("adapter_version", json!(1)),
        ("invocation", json!([])),
        ("operation_key", json!({})),
        ("generation", json!(1)),
        ("namespace", json!("docker")),
    ] {
        let mut binding = binding_value();
        binding[field] = value;
        cases.push(json!({"binding": binding, "operation": inspect_op(ID_A)}));
    }
    for (field, value) in [
        ("engine", json!("Docker")),
        ("endpoint", json!(1)),
        ("context", json!([])),
        ("principal", json!(true)),
        ("rootless", json!("true")),
        ("scope", json!(1)),
    ] {
        let mut binding = binding_value();
        binding["namespace"][field] = value;
        cases.push(json!({"binding": binding, "operation": inspect_op(ID_A)}));
    }
    for case in cases {
        reject_request(&case);
    }
    let request = request(&request_value(inspect_op(ID_A)));
    let binding = binding_value();
    for outcome in [
        json!({"present": {"object": {"object_id": ID_A, "labels": "nope"}}}),
        json!({"present": {"object": {"object_id": ID_A, "labels": [{"key": 1, "value": "v"}]}}}),
        json!({"capabilities": {"supported": "inspect"}}),
        json!({"capabilities": {"supported": [1]}}),
    ] {
        reject_response(
            &response_value(&binding, inspect_op(ID_A), outcome),
            &request,
        );
    }
}

#[test]
fn object_ids_require_exact_lowercase_hex() {
    for id in [ID_A, ID_B] {
        assert!(parse_request(&bytes(&request_value(inspect_op(id)))).is_ok());
        assert!(parse_request(&bytes(&request_value(remove_op(id)))).is_ok());
    }
    for bad in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        format!("sha256:{}", "a".repeat(64)),
        "g".repeat(64),
        String::new(),
    ] {
        reject_request(&request_value(inspect_op(&bad)));
        reject_request(&request_value(remove_op(&bad)));
    }
    let request = request(&request_value(inspect_op(ID_A)));
    reject_response(
        &response_value(&binding_value(), inspect_op(ID_A), present_outcome(ID_B)),
        &request,
    );
    let mut uppercase = present_outcome(ID_A);
    uppercase["present"]["object"]["object_id"] = json!(ID_A.to_uppercase());
    reject_response(
        &response_value(&binding_value(), inspect_op(ID_A), uppercase),
        &request,
    );
}

#[test]
fn identity_strings_reject_empty_and_control_characters() {
    for (field, path) in [
        ("adapter_version", "top"),
        ("invocation", "top"),
        ("operation_key", "top"),
        ("generation", "top"),
        ("endpoint", "namespace"),
        ("context", "namespace"),
        ("principal", "namespace"),
        ("scope", "namespace"),
    ] {
        for value in ["", " ", "\u{0}", "line\nfeed"] {
            let mut binding = binding_value();
            if path == "top" {
                binding[field] = json!(value);
            } else {
                binding["namespace"][field] = json!(value);
            }
            reject_request(&json!({"binding": binding, "operation": inspect_op(ID_A)}));
        }
    }
}

#[test]
fn message_size_boundary_is_enforced() {
    let base = raw_request(&format!(r#"{{"inspect":{{"object_id":"{ID_A}"}}}}"#));
    let pad = MAX_MESSAGE_BYTES.saturating_sub(base.len());
    assert!(pad > 1);
    let exact = format!("{base}{}", " ".repeat(pad));
    assert_eq!(exact.len(), MAX_MESSAGE_BYTES);
    assert!(parse_request(exact.as_bytes()).is_ok());
    let over = format!("{exact} ");
    assert_eq!(over.len(), MAX_MESSAGE_BYTES + 1);
    assert!(parse_request(over.as_bytes()).is_err());
}

#[test]
fn response_binding_must_match_every_field() {
    let expected = request(&request_value(inspect_op(ID_A)));
    let operation = inspect_op(ID_A);
    let outcome = json!("absent");
    let valid = response_value(&binding_value(), operation.clone(), outcome.clone());
    assert!(parse_response(&bytes(&valid), &expected).is_ok());

    let mutations: [fn(&mut Value); 12] = [
        |b| b["protocol"] = json!("gate-runtime/v2"),
        |b| b["adapter_sha256"] = json!(ID_B),
        |b| b["adapter_version"] = json!("1.0.1"),
        |b| b["invocation"] = json!("inv-2"),
        |b| b["operation_key"] = json!("op-2"),
        |b| b["generation"] = json!("gen-2"),
        |b| b["namespace"]["engine"] = json!("podman"),
        |b| b["namespace"]["endpoint"] = json!("unix:///run/podman.sock"),
        |b| b["namespace"]["context"] = json!("ctx-2"),
        |b| b["namespace"]["principal"] = json!("uid:1001"),
        |b| b["namespace"]["rootless"] = json!(false),
        |b| b["namespace"]["scope"] = json!("scope-2"),
    ];
    for mutate in mutations {
        let mut binding = binding_value();
        mutate(&mut binding);
        reject_response(
            &response_value(&binding, operation.clone(), outcome.clone()),
            &expected,
        );
    }
    reject_response(
        &response_value(&binding_value(), inspect_op(ID_B), json!("absent")),
        &expected,
    );
    reject_response(
        &response_value(&binding_value(), remove_op(ID_A), json!("completed")),
        &expected,
    );
}

#[test]
fn operation_and_outcome_matrix_rejects_contradictions() {
    let binding = binding_value();
    let inspect = request(&request_value(inspect_op(ID_A)));
    for outcome in [
        json!("completed"),
        json!("rejected-before-effect"),
        json!("unknown"),
        json!({"capabilities": {"supported": ["inspect"]}}),
    ] {
        reject_response(
            &response_value(&binding, inspect_op(ID_A), outcome),
            &inspect,
        );
    }
    let valid_inspect = [json!("absent"), json!("unavailable"), present_outcome(ID_A)];
    for outcome in valid_inspect {
        assert!(parse_response(
            &bytes(&response_value(&binding, inspect_op(ID_A), outcome)),
            &inspect
        )
        .is_ok());
    }
    let remove = request(&request_value(remove_op(ID_A)));
    for outcome in [
        json!("absent"),
        json!("unavailable"),
        present_outcome(ID_A),
        json!({"capabilities": {"supported": ["remove"]}}),
    ] {
        reject_response(&response_value(&binding, remove_op(ID_A), outcome), &remove);
    }
    for outcome in [
        json!("completed"),
        json!("rejected-before-effect"),
        json!("unknown"),
    ] {
        assert!(parse_response(
            &bytes(&response_value(&binding, remove_op(ID_A), outcome)),
            &remove
        )
        .is_ok());
    }
    let capabilities = request(&request_value(json!("capabilities")));
    for outcome in [
        json!("absent"),
        json!("unavailable"),
        json!("completed"),
        json!("rejected-before-effect"),
        json!("unknown"),
        present_outcome(ID_A),
    ] {
        reject_response(
            &response_value(&binding, json!("capabilities"), outcome),
            &capabilities,
        );
    }
}

#[test]
fn labels_are_unique_nonempty_and_ownership_is_subset_checked() {
    let expected = request(&request_value(inspect_op(ID_A)));
    for labels in [
        json!([]),
        json!([{"key": "k", "value": "v"}, {"key": "k", "value": "v2"}]),
        json!([{"key": "", "value": "v"}]),
        json!([{"key": "k", "value": ""}]),
        json!([{"key": "k", "value": "line\nfeed"}]),
    ] {
        let outcome = json!({"present": {"object": {"object_id": ID_A, "labels": labels}}});
        reject_response(
            &response_value(&binding_value(), inspect_op(ID_A), outcome),
            &expected,
        );
    }
    let valid = response(
        &response_value(&binding_value(), inspect_op(ID_A), present_outcome(ID_A)),
        &expected,
    );
    let required = [Label {
        key: "harness-gate.owner".into(),
        value: "harness-gate".into(),
    }];
    assert!(valid.require_ownership(&expected, &required).is_ok());
    let extended_outcome = json!({"present": {"object": {
        "object_id": ID_A,
        "labels": [
            {"key": "harness-gate.owner", "value": "harness-gate"},
            {"key": "extra", "value": "allowed"}
        ]
    }}});
    let extended = response(
        &response_value(&binding_value(), inspect_op(ID_A), extended_outcome),
        &expected,
    );
    assert!(extended.require_ownership(&expected, &required).is_ok());
    let extra_expected = [
        Label {
            key: "harness-gate.owner".into(),
            value: "harness-gate".into(),
        },
        Label {
            key: "extra".into(),
            value: "allowed".into(),
        },
    ];
    for missing in [
        vec![Label {
            key: "harness-gate.owner".into(),
            value: "other".into(),
        }],
        vec![Label {
            key: "missing".into(),
            value: "v".into(),
        }],
        Vec::new(),
        vec![
            Label {
                key: "harness-gate.owner".into(),
                value: "harness-gate".into(),
            },
            Label {
                key: "harness-gate.owner".into(),
                value: "harness-gate".into(),
            },
        ],
        extra_expected.to_vec(),
    ] {
        assert!(valid.require_ownership(&expected, &missing).is_err());
    }
    let absent = response(
        &response_value(&binding_value(), inspect_op(ID_A), json!("absent")),
        &expected,
    );
    assert!(absent.require_ownership(&expected, &required).is_err());
}

#[test]
fn capabilities_are_unique_and_nonempty() {
    let request = request(&request_value(json!("capabilities")));
    for supported in [
        json!([]),
        json!(["inspect", "inspect"]),
        json!(["start"]),
        json!([]),
    ] {
        reject_response(
            &response_value(
                &binding_value(),
                json!("capabilities"),
                json!({"capabilities": {"supported": supported}}),
            ),
            &request,
        );
    }
    let valid = response(
        &response_value(
            &binding_value(),
            json!("capabilities"),
            json!({"capabilities": {"supported": ["inspect", "remove"]}}),
        ),
        &request,
    );
    assert!(valid.validate(&request).is_ok());
}

#[test]
fn require_same_intent_rejects_any_change() {
    let current = request(&request_value(inspect_op(ID_A)));
    assert!(current.require_same_intent(&current.clone()).is_ok());
    let mut changed = current.clone();
    changed.binding.adapter_sha256 = ID_B.into();
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.binding.adapter_version = "2".into();
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.binding.invocation = "inv-2".into();
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.binding.operation_key = "op-2".into();
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.binding.generation = "gen-2".into();
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.binding.namespace.endpoint = "unix:///other".into();
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.binding.namespace.rootless = false;
    assert!(current.require_same_intent(&changed).is_err());
    let mut changed = current.clone();
    changed.operation = Operation::Remove {
        object_id: ID_A.into(),
    };
    assert!(current.require_same_intent(&changed).is_err());
    let mut invalid = current.clone();
    invalid.binding.invocation.clear();
    assert!(current.require_same_intent(&invalid).is_err());
}

#[test]
fn require_absence_needs_separate_exact_id_inspection() {
    let remove = request(&request_value(remove_op(ID_A)));
    let inspect_expected = Request {
        binding: remove.binding.clone(),
        operation: Operation::Inspect {
            object_id: ID_A.into(),
        },
    };
    let binding = binding_value();
    let inspect = inspect_op(ID_A);
    let absent = response(
        &response_value(&binding, inspect.clone(), json!("absent")),
        &inspect_expected,
    );
    assert!(require_absence(&remove, &absent).is_ok());

    let unavailable = response(
        &response_value(&binding, inspect.clone(), json!("unavailable")),
        &inspect_expected,
    );
    assert!(require_absence(&remove, &unavailable).is_err());
    let present = response(
        &response_value(&binding, inspect.clone(), present_outcome(ID_A)),
        &inspect_expected,
    );
    assert!(require_absence(&remove, &present).is_err());

    let inspect_expected_b = Request {
        binding: remove.binding.clone(),
        operation: Operation::Inspect {
            object_id: ID_B.into(),
        },
    };
    let wrong_id = response(
        &response_value(&binding, inspect_op(ID_B), json!("absent")),
        &inspect_expected_b,
    );
    assert!(require_absence(&remove, &wrong_id).is_err());

    let mut changed_generation = absent.clone();
    changed_generation.binding.generation = "gen-2".into();
    assert!(require_absence(&remove, &changed_generation).is_err());
    let mut changed_namespace = absent.clone();
    changed_namespace.binding.namespace.context = "ctx-2".into();
    assert!(require_absence(&remove, &changed_namespace).is_err());
    let mut changed_key = absent.clone();
    changed_key.binding.operation_key = "op-2".into();
    assert!(require_absence(&remove, &changed_key).is_err());

    let completed = response(
        &response_value(&binding, remove_op(ID_A), json!("completed")),
        &remove,
    );
    assert!(require_absence(&remove, &completed).is_err());

    let not_remove = request(&request_value(inspect_op(ID_A)));
    assert!(require_absence(&not_remove, &absent).is_err());
}

#[test]
fn legacy_v2_lease_json_is_rejected_without_backfill() {
    let legacy = json!({
        "owner_marker": "harness-gate",
        "schema_version": 2,
        "project_identity": "sha256:project",
        "resource_id": "service:postgres",
        "resource_kind": "container",
        "invocation_id": "inv-1",
        "pid": 1000,
        "process_start_identity": "linux:1",
        "created_at": 1,
        "heartbeat_at": 1,
        "expires_at": 2,
        "resource_name": "harness-gate-postgres",
        "runtime": "docker",
        "runtime_labels": {"harness-gate.owner": "harness-gate"},
        "runtime_object_id": ID_A
    });
    assert!(parse_request(&bytes(&legacy)).is_err());
    for missing in [
        "adapter_sha256",
        "adapter_version",
        "invocation",
        "operation_key",
        "generation",
        "namespace",
    ] {
        let mut binding = binding_value();
        binding.as_object_mut().unwrap().remove(missing);
        reject_request(&json!({"binding": binding, "operation": inspect_op(ID_A)}));
    }
    let mut namespace = binding_value();
    namespace["namespace"]
        .as_object_mut()
        .unwrap()
        .remove("principal");
    reject_request(&json!({"binding": namespace, "operation": inspect_op(ID_A)}));
}

#[test]
fn errors_do_not_echo_untrusted_values() {
    let sentinel = "SECRET-ECHO-SENTINEL";
    let cases = [
        format!(r#"{{"binding":{{}},"operation":"{sentinel}"}}"#),
        raw_request(&format!(
            r#"{{"inspect":{{"object_id":"{ID_A}","leak":"{sentinel}"}}}}"#
        )),
        raw_request(&format!(r#"{{"inspect":{{"object_id":"{sentinel}"}}}}"#)),
        sentinel.to_string(),
    ];
    for text in cases {
        let error = parse_request(text.as_bytes()).expect_err("invalid message");
        assert!(!error.to_string().contains(sentinel), "{error}");
    }
    let mut binding = binding_value();
    binding["invocation"] = json!(format!("{sentinel}\n"));
    let request = json!({"binding": binding, "operation": inspect_op(ID_A)});
    let error = parse_request(&bytes(&request)).expect_err("invalid identity");
    assert!(!error.to_string().contains(sentinel), "{error}");
}

#[test]
fn canonical_unit_and_struct_shapes_roundtrip() {
    let capabilities = request(&request_value(json!("capabilities")));
    let text = serde_json::to_string(&capabilities).unwrap();
    assert!(text.contains(r#""operation":"capabilities""#), "{text}");
    assert!(parse_request(text.as_bytes()).is_ok());

    let inspect = request(&request_value(inspect_op(ID_A)));
    let text = serde_json::to_string(&inspect).unwrap();
    assert!(text.contains(r#""inspect":{"object_id""#), "{text}");
    assert!(parse_request(text.as_bytes()).is_ok());

    let absent = response(
        &response_value(&binding_value(), inspect_op(ID_A), json!("absent")),
        &inspect,
    );
    let text = serde_json::to_string(&absent).unwrap();
    assert!(text.contains(r#""outcome":"absent""#), "{text}");
    assert!(parse_response(text.as_bytes(), &inspect).is_ok());

    let supported = response(
        &response_value(
            &binding_value(),
            json!("capabilities"),
            json!({"capabilities": {"supported": ["inspect"]}}),
        ),
        &capabilities,
    );
    let text = serde_json::to_string(&supported).unwrap();
    assert!(
        text.contains(r#""outcome":{"capabilities":{"supported":["inspect"]}}"#),
        "{text}"
    );
}

#[test]
fn protocol_engine_and_capability_map_encodings_are_rejected() {
    let mut binding = binding_value();
    binding["protocol"] = json!({"gate-runtime/v1": null});
    reject_request(&json!({"binding": binding, "operation": inspect_op(ID_A)}));

    let mut binding = binding_value();
    binding["namespace"]["engine"] = json!({"docker": null});
    reject_request(&json!({"binding": binding, "operation": inspect_op(ID_A)}));

    let request = request(&request_value(json!("capabilities")));
    reject_response(
        &response_value(
            &binding_value(),
            json!("capabilities"),
            json!({"capabilities": {"supported": [{"inspect": null}]}}),
        ),
        &request,
    );
}

/// Unit operations must have exactly one canonical string encoding, never a
/// map carrying `null`.
#[test]
fn unit_operation_map_null_encoding_is_rejected() {
    let binding = serde_json::to_string(&binding_value()).unwrap();
    let text = format!(r#"{{"binding":{binding},"operation":{{"capabilities":null}}}}"#);
    assert!(
        parse_request(text.as_bytes()).is_err(),
        "accepted map+null unit operation"
    );
}

/// Unit outcomes must have exactly one canonical string encoding, never a map
/// carrying `null`.
#[test]
fn unit_outcome_map_null_encoding_is_rejected() {
    let inspect = request(&request_value(inspect_op(ID_A)));
    let binding = serde_json::to_string(&binding_value()).unwrap();
    let mut accepted = Vec::new();
    for outcome in ["absent", "unavailable"] {
        let text = format!(
            r#"{{"binding":{binding},"operation":{{"inspect":{{"object_id":"{ID_A}"}}}},"outcome":{{"{outcome}":null}}}}"#
        );
        if parse_response(text.as_bytes(), &inspect).is_ok() {
            accepted.push(format!("inspect:{outcome}"));
        }
    }
    let remove = request(&request_value(remove_op(ID_A)));
    for outcome in ["completed", "rejected-before-effect", "unknown"] {
        let text = format!(
            r#"{{"binding":{binding},"operation":{{"remove":{{"object_id":"{ID_A}"}}}},"outcome":{{"{outcome}":null}}}}"#
        );
        if parse_response(text.as_bytes(), &remove).is_ok() {
            accepted.push(format!("remove:{outcome}"));
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted map+null unit outcome encodings: {accepted:?}"
    );
}
