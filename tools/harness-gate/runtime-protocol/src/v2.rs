//! Explicit Docker Unix-HTTP identity contract. No I/O or daemon attestation.
//! Independent of v1: no automatic request, lease or persisted-intent conversion.
use crate::{digest, nonempty, validate_labels, Invalid, Label, Operation, Outcome, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

macro_rules! string_identity {
    ($name:ident, $variant:ident, $wire:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub enum $name {
            #[serde(rename = $wire)]
            $variant,
        }
        impl TryFrom<String> for $name {
            type Error = Invalid;
            fn try_from(value: String) -> Result<Self> {
                if value == $wire {
                    Ok(Self::$variant)
                } else {
                    Err(Invalid("unsupported v2 identity"))
                }
            }
        }
    };
}
string_identity!(Protocol, V2, "gate-runtime/v2");
string_identity!(Engine, Docker, "docker");
string_identity!(Transport, UnixHttp, "unix-http");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub protocol: Protocol,
    pub adapter_sha256: String,
    pub adapter_version: String,
    pub invocation: String,
    pub operation_key: String,
    pub generation: String,
    pub adapter_config_sha256: String,
    pub engine: Engine,
    pub transport: Transport,
    pub socket_path: String,
    pub socket_uid: u32,
    pub principal_uid: u32,
    /// Configured expectation, not independently authenticated rootless proof.
    pub rootless: bool,
    pub api_version: String,
    pub server_version: String,
    pub daemon_id: String,
    /// Ownership scope only; never an encoding for missing engine pins.
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub binding: Binding,
    pub operation: Operation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedIdentity {
    pub api_version: String,
    pub server_version: String,
    pub daemon_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    pub observed_before: ObservedIdentity,
    pub observed_after: ObservedIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub binding: Binding,
    pub operation: Operation,
    pub outcome: Outcome,
    // Explicit null is required for non-inspection facts, not a missing field.
    #[serde(deserialize_with = "required_observations")]
    pub observations: Option<Observations>,
}

fn required_observations<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Observations>, D::Error> {
    Option::deserialize(deserializer)
}

impl Binding {
    pub fn validate(&self) -> Result<()> {
        digest(&self.adapter_sha256)?;
        digest(&self.adapter_config_sha256)?;
        for value in [
            &self.adapter_version,
            &self.invocation,
            &self.operation_key,
            &self.generation,
            &self.server_version,
            &self.daemon_id,
            &self.scope,
        ] {
            nonempty(value)?;
        }
        validate_socket_path(&self.socket_path)?;
        validate_api_version(&self.api_version)
    }
}

fn validate_socket_path(path: &str) -> Result<()> {
    nonempty(path)?;
    if !path.starts_with('/')
        || path[1..]
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(Invalid("expected canonical absolute Unix socket path"));
    }
    Ok(())
}

fn validate_api_version(version: &str) -> Result<()> {
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 2
        || parts.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|b| b.is_ascii_digit())
                || part.parse::<u16>().is_err()
        })
    {
        return Err(Invalid("expected numeric major.minor API version"));
    }
    Ok(())
}

impl Request {
    pub fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        match &self.operation {
            Operation::Capabilities => Ok(()),
            Operation::Inspect { object_id } | Operation::Remove { object_id } => digest(object_id),
        }
    }

    pub fn require_same_intent(&self, previous: &Self) -> Result<()> {
        self.validate()?;
        previous.validate()?;
        if self != previous {
            return Err(Invalid("v2 request differs from recorded intent"));
        }
        Ok(())
    }
}

impl ObservedIdentity {
    fn matches(&self, binding: &Binding) -> bool {
        self.api_version == binding.api_version
            && self.server_version == binding.server_version
            && self.daemon_id == binding.daemon_id
    }
}

impl Response {
    pub fn validate(&self, expected: &Request) -> Result<()> {
        expected.validate()?;
        if self.binding != expected.binding || self.operation != expected.operation {
            return Err(Invalid("v2 response binding mismatch"));
        }
        self.validate_observations()?;
        match (&self.operation, &self.outcome) {
            (Operation::Capabilities, Outcome::Capabilities { supported }) => {
                let unique: BTreeSet<_> = supported.iter().collect();
                if supported.is_empty() || unique.len() != supported.len() {
                    return Err(Invalid("empty or duplicate capabilities"));
                }
            }
            (Operation::Inspect { object_id }, Outcome::Present { object }) => {
                if object.object_id != *object_id {
                    return Err(Invalid("object identity mismatch"));
                }
                validate_labels(&object.labels)?;
            }
            (Operation::Inspect { .. }, Outcome::Absent | Outcome::Unavailable) => {}
            (
                Operation::Remove { .. },
                Outcome::Completed | Outcome::RejectedBeforeEffect | Outcome::Unknown,
            ) => {}
            _ => return Err(Invalid("outcome incompatible with operation")),
        }
        Ok(())
    }

    fn validate_observations(&self) -> Result<()> {
        match (&self.outcome, &self.observations) {
            (Outcome::Present { .. } | Outcome::Absent, Some(pair))
                if pair.observed_before.matches(&self.binding)
                    && pair.observed_after.matches(&self.binding) =>
            {
                Ok(())
            }
            (Outcome::Present { .. } | Outcome::Absent, _) => Err(Invalid(
                "matching before and after engine observations required",
            )),
            (_, None) => Ok(()),
            (_, Some(_)) => Err(Invalid("observations incompatible with outcome")),
        }
    }

    pub fn require_ownership(&self, expected: &Request, labels: &[Label]) -> Result<()> {
        self.validate(expected)?;
        validate_labels(labels)?;
        let Outcome::Present { object } = &self.outcome else {
            return Err(Invalid("ownership requires a present object"));
        };
        if !labels.iter().all(|label| object.labels.contains(label)) {
            return Err(Invalid("ownership labels mismatch"));
        }
        Ok(())
    }
}

pub fn parse_request(bytes: &[u8]) -> Result<Request> {
    let request: Request = crate::parse(bytes)?;
    request.validate()?;
    Ok(request)
}

pub fn parse_response(bytes: &[u8], expected: &Request) -> Result<Response> {
    let response: Response = crate::parse(bytes)?;
    response.validate(expected)?;
    Ok(response)
}

pub fn require_absence(remove: &Request, inspection: &Response) -> Result<()> {
    remove.validate()?;
    let Operation::Remove { object_id } = &remove.operation else {
        return Err(Invalid("reconciliation requires a removal intent"));
    };
    let expected = Request {
        binding: remove.binding.clone(),
        operation: Operation::Inspect {
            object_id: object_id.clone(),
        },
    };
    inspection.validate(&expected)?;
    if inspection.outcome != Outcome::Absent {
        return Err(Invalid("exact object absence not established"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_request as parse_v1_request, Capability, Object, Operation, Outcome};
    use serde_json::{json, Value};

    fn binding() -> Binding {
        Binding {
            protocol: Protocol::V2,
            adapter_sha256: "a".repeat(64),
            adapter_version: "1.0.0".into(),
            invocation: "inv-1".into(),
            operation_key: "op-1".into(),
            generation: "gen-1".into(),
            adapter_config_sha256: "b".repeat(64),
            engine: Engine::Docker,
            transport: Transport::UnixHttp,
            socket_path: "/run/docker.sock".into(),
            socket_uid: 0,
            principal_uid: 1000,
            rootless: false,
            api_version: "1.47".into(),
            server_version: "27.0.1".into(),
            daemon_id: "daemon-1".into(),
            scope: "scope-1".into(),
        }
    }

    fn id() -> String {
        "c".repeat(64)
    }

    fn inspect_request() -> Request {
        Request {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
        }
    }

    fn remove_request() -> Request {
        Request {
            binding: binding(),
            operation: Operation::Remove { object_id: id() },
        }
    }

    fn observed() -> ObservedIdentity {
        ObservedIdentity {
            api_version: "1.47".into(),
            server_version: "27.0.1".into(),
            daemon_id: "daemon-1".into(),
        }
    }

    fn observations() -> Observations {
        Observations {
            observed_before: observed(),
            observed_after: observed(),
        }
    }

    fn present() -> Outcome {
        Outcome::Present {
            object: Object {
                object_id: id(),
                labels: vec![Label {
                    key: "harness-gate.owner".into(),
                    value: "harness-gate".into(),
                }],
            },
        }
    }

    fn response(
        operation: Operation,
        outcome: Outcome,
        observations: Option<Observations>,
    ) -> Response {
        Response {
            binding: binding(),
            operation,
            outcome,
            observations,
        }
    }

    fn bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }

    fn request_value(request: &Request) -> Value {
        serde_json::to_value(request).unwrap()
    }

    fn parse_ok(value: &Value, expected: &Request) -> Response {
        parse_response(&bytes(value), expected).expect("valid v2 response")
    }

    #[test]
    fn accepts_valid_v2_shapes() {
        let inspect = inspect_request();
        for outcome in [Outcome::Absent, Outcome::Unavailable, present()] {
            let observations = if matches!(outcome, Outcome::Unavailable) {
                None
            } else {
                Some(observations())
            };
            parse_ok(
                &serde_json::to_value(response(inspect.operation.clone(), outcome, observations))
                    .unwrap(),
                &inspect,
            );
        }
        let remove = remove_request();
        for outcome in [
            Outcome::Completed,
            Outcome::RejectedBeforeEffect,
            Outcome::Unknown,
        ] {
            parse_ok(
                &serde_json::to_value(response(remove.operation.clone(), outcome, None)).unwrap(),
                &remove,
            );
        }
        let capabilities = Request {
            binding: binding(),
            operation: Operation::Capabilities,
        };
        parse_ok(
            &serde_json::to_value(response(
                Operation::Capabilities,
                Outcome::Capabilities {
                    supported: vec![Capability::Inspect, Capability::Remove],
                },
                None,
            ))
            .unwrap(),
            &capabilities,
        );
    }

    #[test]
    fn observations_are_required_and_strict() {
        let request = inspect_request();
        let base = serde_json::to_value(response(
            request.operation.clone(),
            Outcome::Absent,
            Some(observations()),
        ))
        .unwrap();
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("observations");
        assert!(parse_response(&bytes(&missing), &request).is_err());

        let mut null = base.clone();
        null["observations"] = json!(null);
        assert!(parse_response(&bytes(&null), &request).is_err());

        let mut missing_after = base.clone();
        missing_after["observations"]
            .as_object_mut()
            .unwrap()
            .remove("observed_after");
        assert!(parse_response(&bytes(&missing_after), &request).is_err());

        let mut duplicate = base.clone();
        duplicate["observations"]["extra"] = json!(1);
        assert!(parse_response(&bytes(&duplicate), &request).is_err());
        let raw = serde_json::to_string(&base).unwrap();
        let duplicate_key = raw.replacen(
            r#""observations":{"#,
            r#""observations":null,"observations":{"#,
            1,
        );
        assert!(parse_response(duplicate_key.as_bytes(), &request).is_err());

        let mut missing_before = base.clone();
        missing_before["observations"]
            .as_object_mut()
            .unwrap()
            .remove("observed_before");
        assert!(parse_response(&bytes(&missing_before), &request).is_err());

        let mut unknown_observed = base.clone();
        unknown_observed["observations"]["observed_before"]["extra"] = json!(1);
        assert!(parse_response(&bytes(&unknown_observed), &request).is_err());

        let mut before = base;
        before["observations"]["observed_before"]["daemon_id"] = json!("other");
        assert!(parse_response(&bytes(&before), &request).is_err());

        let remove = remove_request();
        let completed =
            serde_json::to_value(response(remove.operation.clone(), Outcome::Completed, None))
                .unwrap();
        parse_ok(&completed, &remove);
        let mut null_ok = completed.clone();
        null_ok["observations"] = json!(null);
        parse_ok(&null_ok, &remove);
        let mut missing_ok = completed;
        missing_ok.as_object_mut().unwrap().remove("observations");
        assert!(parse_response(&bytes(&missing_ok), &remove).is_err());
        let mut some_completed = serde_json::to_value(response(
            remove.operation.clone(),
            Outcome::Completed,
            Some(observations()),
        ))
        .unwrap();
        assert!(parse_response(&bytes(&some_completed), &remove).is_err());
        some_completed["observations"] = json!([]);
        assert!(parse_response(&bytes(&some_completed), &remove).is_err());
    }

    #[test]
    fn binding_pins_and_docker_only_are_strict() {
        for (name, value) in [
            ("protocol", json!({"gate-runtime/v2": null})),
            ("engine", json!({"docker": null})),
            ("transport", json!({"unix-http": null})),
        ] {
            let mut request = request_value(&inspect_request());
            request["binding"][name] = value;
            assert!(parse_request(&bytes(&request)).is_err(), "{name}");
        }
        let mut missing = request_value(&inspect_request());
        missing["binding"]
            .as_object_mut()
            .unwrap()
            .remove("daemon_id");
        assert!(parse_request(&bytes(&missing)).is_err());
        let mut unknown = request_value(&inspect_request());
        unknown["binding"]["extra"] = json!("x");
        assert!(parse_request(&bytes(&unknown)).is_err());
        let raw = serde_json::to_string(&request_value(&inspect_request())).unwrap();
        let duplicate_binding = raw.replacen(r#""binding":{"#, r#""binding":null,"binding":{"#, 1);
        assert!(parse_request(duplicate_binding.as_bytes()).is_err());

        for (name, value) in [
            ("protocol", json!("gate-runtime/v1")),
            ("engine", json!("podman")),
            ("transport", json!("cli")),
            ("adapter_sha256", json!("A".repeat(64))),
            ("adapter_config_sha256", json!("short")),
            ("socket_path", json!("relative/docker.sock")),
            ("socket_path", json!("/run/../docker.sock")),
            ("socket_path", json!("/run//docker.sock")),
            ("socket_path", json!("/run/docker.sock/")),
            ("api_version", json!("1")),
            ("api_version", json!("01.47")),
            ("api_version", json!("1.2.3")),
            ("api_version", json!("x.47")),
            ("socket_uid", json!("0")),
            ("principal_uid", json!(-1)),
            ("rootless", json!("false")),
            ("daemon_id", json!("")),
            ("server_version", json!("line\nfeed")),
            ("scope", json!("")),
        ] {
            let mut request = request_value(&inspect_request());
            request["binding"][name] = value;
            assert!(parse_request(&bytes(&request)).is_err(), "{name}");
        }
    }

    #[test]
    fn every_binding_change_is_rejected() {
        let expected = inspect_request();
        let base = serde_json::to_value(response(
            expected.operation.clone(),
            Outcome::Absent,
            Some(observations()),
        ))
        .unwrap();
        let mutations = [
            ("protocol", json!("gate-runtime/v1")),
            ("adapter_sha256", json!("d".repeat(64))),
            ("adapter_version", json!("2")),
            ("invocation", json!("inv-2")),
            ("operation_key", json!("op-2")),
            ("generation", json!("gen-2")),
            ("adapter_config_sha256", json!("e".repeat(64))),
            ("engine", json!("podman")),
            ("transport", json!("cli")),
            ("socket_path", json!("/run/other.sock")),
            ("socket_uid", json!(1)),
            ("principal_uid", json!(1001)),
            ("rootless", json!(true)),
            ("api_version", json!("1.48")),
            ("server_version", json!("28")),
            ("daemon_id", json!("daemon-2")),
            ("scope", json!("scope-2")),
        ];
        for (field, value) in mutations {
            let mut changed = base.clone();
            changed["binding"][field] = value;
            assert!(
                parse_response(&bytes(&changed), &expected).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn observations_must_match_before_and_after() {
        let expected = inspect_request();
        for mutation in [
            |v: &mut Value| v["observations"]["observed_before"]["api_version"] = json!("1.48"),
            |v: &mut Value| v["observations"]["observed_before"]["server_version"] = json!("28"),
            |v: &mut Value| v["observations"]["observed_before"]["daemon_id"] = json!("other"),
            |v: &mut Value| v["observations"]["observed_after"]["api_version"] = json!("1.48"),
            |v: &mut Value| v["observations"]["observed_after"]["server_version"] = json!("28"),
            |v: &mut Value| v["observations"]["observed_after"]["daemon_id"] = json!("other"),
        ] {
            let mut value = serde_json::to_value(response(
                expected.operation.clone(),
                Outcome::Absent,
                Some(observations()),
            ))
            .unwrap();
            mutation(&mut value);
            assert!(parse_response(&bytes(&value), &expected).is_err());
        }
    }

    #[test]
    fn outcome_matrix_and_observations_are_consistent() {
        let inspect = inspect_request();
        for outcome in [
            Outcome::Completed,
            Outcome::RejectedBeforeEffect,
            Outcome::Unknown,
        ] {
            let value =
                serde_json::to_value(response(inspect.operation.clone(), outcome, None)).unwrap();
            assert!(parse_response(&bytes(&value), &inspect).is_err());
        }
        let remove = remove_request();
        for outcome in [present(), Outcome::Absent, Outcome::Unavailable] {
            let value = serde_json::to_value(response(
                remove.operation.clone(),
                outcome,
                Some(observations()),
            ))
            .unwrap();
            assert!(parse_response(&bytes(&value), &remove).is_err());
        }
        let capabilities = Request {
            binding: binding(),
            operation: Operation::Capabilities,
        };
        for outcome in [
            Outcome::Absent,
            Outcome::Unavailable,
            present(),
            Outcome::Completed,
        ] {
            let value =
                serde_json::to_value(response(Operation::Capabilities, outcome, None)).unwrap();
            assert!(parse_response(&bytes(&value), &capabilities).is_err());
        }
        let mut wrong_object = serde_json::to_value(response(
            inspect.operation.clone(),
            present(),
            Some(observations()),
        ))
        .unwrap();
        wrong_object["outcome"]["present"]["object"]["object_id"] = json!(1);
        assert!(parse_response(&bytes(&wrong_object), &inspect).is_err());

        let empty = serde_json::to_value(response(
            Operation::Capabilities,
            Outcome::Capabilities { supported: vec![] },
            None,
        ))
        .unwrap();
        assert!(parse_response(&bytes(&empty), &capabilities).is_err());
        let duplicate = serde_json::to_value(response(
            Operation::Capabilities,
            Outcome::Capabilities {
                supported: vec![Capability::Inspect, Capability::Inspect],
            },
            None,
        ))
        .unwrap();
        assert!(parse_response(&bytes(&duplicate), &capabilities).is_err());
    }

    #[test]
    fn ownership_and_absence_helpers() {
        let expected = inspect_request();
        let presented = parse_ok(
            &serde_json::to_value(response(
                expected.operation.clone(),
                present(),
                Some(observations()),
            ))
            .unwrap(),
            &expected,
        );
        let required = vec![Label {
            key: "harness-gate.owner".into(),
            value: "harness-gate".into(),
        }];
        assert!(presented.require_ownership(&expected, &required).is_ok());
        let extra = vec![
            Label {
                key: "harness-gate.owner".into(),
                value: "harness-gate".into(),
            },
            Label {
                key: "missing".into(),
                value: "v".into(),
            },
        ];
        assert!(presented.require_ownership(&expected, &extra).is_err());
        assert!(presented.require_ownership(&expected, &[]).is_err());
        let duplicate = vec![required[0].clone(), required[0].clone()];
        assert!(presented.require_ownership(&expected, &duplicate).is_err());
        let absent = parse_ok(
            &serde_json::to_value(response(
                expected.operation.clone(),
                Outcome::Absent,
                Some(observations()),
            ))
            .unwrap(),
            &expected,
        );
        assert!(absent.require_ownership(&expected, &required).is_err());

        let remove = remove_request();
        assert!(require_absence(&remove, &absent).is_ok());
        assert!(require_absence(&expected, &absent).is_err());
        let present_response = parse_ok(
            &serde_json::to_value(response(
                expected.operation.clone(),
                present(),
                Some(observations()),
            ))
            .unwrap(),
            &expected,
        );
        assert!(require_absence(&remove, &present_response).is_err());
        let mut wrong = absent.clone();
        wrong.binding.generation = "gen-2".into();
        assert!(require_absence(&remove, &wrong).is_err());
        let mut wrong_observations = absent;
        wrong_observations
            .observations
            .as_mut()
            .unwrap()
            .observed_after
            .daemon_id = "other".into();
        assert!(require_absence(&remove, &wrong_observations).is_err());
    }

    #[test]
    fn v1_and_v2_reject_each_other() {
        let v2 = bytes(&request_value(&inspect_request()));
        assert!(parse_v1_request(&v2).is_err());
        let v1 = br#"{"binding":{"protocol":"gate-runtime/v1","adapter_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","adapter_version":"1","invocation":"i","operation_key":"k","generation":"g","namespace":{"engine":"docker","endpoint":"unix:///run/docker.sock","context":"ctx","principal":"uid:1000","rootless":false,"scope":"s"}},"operation":"capabilities"}"#;
        assert!(parse_request(v1).is_err());
    }

    #[test]
    fn size_and_errors_do_not_echo_input() {
        let request = bytes(&request_value(&inspect_request()));
        let mut oversized = request.clone();
        oversized.extend(std::iter::repeat_n(b' ', crate::MAX_MESSAGE_BYTES));
        assert!(parse_request(&oversized).is_err());
        let sentinel = "SENTINEL-V2";
        let invalid =
            format!(r#"{{"binding":{{"protocol":"{sentinel}"}},"operation":"capabilities"}}"#);
        let error = parse_request(invalid.as_bytes()).unwrap_err();
        assert!(!error.to_string().contains(sentinel));
        let expected = inspect_request();
        let mut value = serde_json::to_value(response(
            expected.operation.clone(),
            Outcome::Absent,
            Some(observations()),
        ))
        .unwrap();
        value["observations"]["observed_before"]["daemon_id"] = json!(sentinel);
        let error = parse_response(&bytes(&value), &expected).unwrap_err();
        assert!(!error.to_string().contains(sentinel));
    }
}
