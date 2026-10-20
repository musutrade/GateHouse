//! Pure transitions for a single removal intent. No filesystem, dispatch,
//! namespace discovery or lease migration. A durable coordinator must commit
//! the returned state under its generation lock BEFORE the corresponding effect.
use crate::{require_absence, Invalid, Label, Operation, Request, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", try_from = "String")]
pub enum Phase {
    Prepared,
    Unknown,
    Completed,
}

impl TryFrom<String> for Phase {
    type Error = Invalid;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "prepared" => Ok(Self::Prepared),
            "unknown" => Ok(Self::Unknown),
            "completed" => Ok(Self::Completed),
            _ => Err(Invalid("unsupported intent phase")),
        }
    }
}

/// Fields are private so ordinary callers cannot skip ownership validation.
/// Restore accepts only an operator-protected state record with independent
/// expected request pins; it does not authenticate disk bytes or prevent rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Intent {
    schema: String,
    request: Request,
    phase: Phase,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    request: Request,
    phase: Phase,
}

const SCHEMA: &str = "gate-runtime-intent/v1";

fn removal(request: &Request) -> Result<(), Invalid> {
    request.validate()?;
    if !matches!(request.operation, Operation::Remove { .. }) {
        return Err(Invalid("intent requires exact-ID removal"));
    }
    Ok(())
}

fn inspection_request(removal: &Request) -> Request {
    let Operation::Remove { object_id } = &removal.operation else {
        unreachable!("private helper called only after removal validation");
    };
    Request {
        binding: removal.binding.clone(),
        operation: Operation::Inspect {
            object_id: object_id.clone(),
        },
    }
}

impl Intent {
    /// The coordinator must independently authenticate the inspection producer
    /// and pin required labels from its lease, never from the adapter response.
    pub fn prepare(
        request: &Request,
        inspection: &Response,
        required_labels: &[Label],
    ) -> Result<Self, Invalid> {
        removal(request)?;
        inspection.require_ownership(&inspection_request(request), required_labels)?;
        Ok(Self {
            schema: SCHEMA.into(),
            request: request.clone(),
            phase: Phase::Prepared,
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Commit this Unknown state before dispatch. A crash between commit and
    /// dispatch is conservative: recovery must inspect, never blindly retry.
    pub fn begin_dispatch(&self, expected: &Request) -> Result<Self, Invalid> {
        self.request.require_same_intent(expected)?;
        if self.phase != Phase::Prepared {
            return Err(Invalid("intent is not prepared for dispatch"));
        }
        let mut next = self.clone();
        next.phase = Phase::Unknown;
        Ok(next)
    }

    /// None represents transport loss/cancellation/error. Every valid mutation
    /// response, including completed/rejected-before-effect, leaves Unknown.
    /// Invalid responses return an error without modifying the original state.
    pub fn observe_dispatch(
        &self,
        expected: &Request,
        response: Option<&Response>,
    ) -> Result<Self, Invalid> {
        self.request.require_same_intent(expected)?;
        if self.phase != Phase::Unknown {
            return Err(Invalid("intent has no unresolved dispatch"));
        }
        if let Some(response) = response {
            response.validate(&self.request)?;
        }
        Ok(self.clone())
    }

    /// Only independently authenticated exact-ID absence can finish Unknown.
    /// This pure operation does not remove a marker or grant cleanup authority.
    pub fn reconcile(&self, expected: &Request, inspection: &Response) -> Result<Self, Invalid> {
        self.request.require_same_intent(expected)?;
        if self.phase != Phase::Unknown {
            return Err(Invalid("intent is not awaiting reconciliation"));
        }
        require_absence(&self.request, inspection)?;
        let mut next = self.clone();
        next.phase = Phase::Completed;
        Ok(next)
    }

    /// Read only trusted persisted state. Legacy lease v2, missing pins, duplicate
    /// fields and unknown schema/phase reject; no ambient values are consulted.
    pub fn restore(bytes: &[u8], expected: &Request) -> Result<Self, Invalid> {
        let record: Record = crate::parse(bytes)?;
        if record.schema != SCHEMA {
            return Err(Invalid("unsupported intent schema"));
        }
        removal(&record.request)?;
        record.request.require_same_intent(expected)?;
        Ok(Self {
            schema: record.schema,
            request: record.request,
            phase: record.phase,
        })
    }
}

#[cfg(test)]
mod tests {
    // Pure transition model only. These tests do not establish durable commit,
    // persistence, locking, rollback protection or dispatch behavior.
    use super::*;
    use crate::{Binding, Engine, Namespace, Object, Outcome, Protocol};
    use serde_json::json;

    fn id() -> String {
        "a".repeat(64)
    }

    fn binding() -> Binding {
        Binding {
            protocol: Protocol::V1,
            adapter_sha256: "b".repeat(64),
            adapter_version: "1.0.0".into(),
            invocation: "inv-1".into(),
            operation_key: "op-1".into(),
            generation: "gen-1".into(),
            namespace: Namespace {
                engine: Engine::Docker,
                endpoint: "unix:///run/docker.sock".into(),
                context: "ctx-1".into(),
                principal: "uid:1000".into(),
                rootless: true,
                scope: "scope-1".into(),
            },
        }
    }

    fn removal() -> Request {
        Request {
            binding: binding(),
            operation: Operation::Remove { object_id: id() },
        }
    }

    fn labels() -> Vec<Label> {
        vec![Label {
            key: "harness-gate.owner".into(),
            value: "harness-gate".into(),
        }]
    }

    fn present() -> Response {
        Response {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
            outcome: Outcome::Present {
                object: Object {
                    object_id: id(),
                    labels: labels(),
                },
            },
        }
    }

    fn absent() -> Response {
        Response {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
            outcome: Outcome::Absent,
        }
    }

    fn prepared() -> Intent {
        Intent::prepare(&removal(), &present(), &labels()).unwrap()
    }

    fn unknown() -> Intent {
        prepared().begin_dispatch(&removal()).unwrap()
    }

    fn binding_mutations() -> Vec<Binding> {
        let mut values = Vec::new();
        let mutations: [fn(&mut Binding); 11] = [
            |b| b.adapter_sha256 = "c".repeat(64),
            |b| b.adapter_version = "2".into(),
            |b| b.invocation = "inv-2".into(),
            |b| b.operation_key = "op-2".into(),
            |b| b.generation = "gen-2".into(),
            |b| b.namespace.engine = Engine::Podman,
            |b| b.namespace.endpoint = "unix:///other".into(),
            |b| b.namespace.context = "ctx-2".into(),
            |b| b.namespace.principal = "uid:1001".into(),
            |b| b.namespace.rootless = false,
            |b| b.namespace.scope = "scope-2".into(),
        ];
        for mutate in mutations {
            let mut value = binding();
            mutate(&mut value);
            values.push(value);
        }
        values
    }

    #[test]
    fn prepare_requires_present_exact_identity_and_trusted_labels() {
        assert_eq!(prepared().phase(), Phase::Prepared);
        assert_eq!(prepared().request(), &removal());
        for response in [
            Response {
                binding: binding(),
                operation: Operation::Inspect {
                    object_id: "b".repeat(64),
                },
                outcome: Outcome::Present {
                    object: Object {
                        object_id: "c".repeat(64),
                        labels: labels(),
                    },
                },
            },
            absent(),
            Response {
                binding: binding(),
                operation: Operation::Inspect { object_id: id() },
                outcome: Outcome::Unavailable,
            },
        ] {
            assert!(Intent::prepare(&removal(), &response, &labels()).is_err());
        }
        let mut wrong_binding = present();
        wrong_binding.binding.invocation = "inv-2".into();
        assert!(Intent::prepare(&removal(), &wrong_binding, &labels()).is_err());
        let missing = Label {
            key: "missing".into(),
            value: "v".into(),
        };
        assert!(Intent::prepare(&removal(), &present(), &[missing]).is_err());
        assert!(Intent::prepare(&removal(), &present(), &[]).is_err());
        assert!(Intent::prepare(
            &removal(),
            &present(),
            &[labels()[0].clone(), labels()[0].clone()]
        )
        .is_err());
        let mut wrong_value = labels();
        wrong_value[0].value = "other".into();
        assert!(Intent::prepare(&removal(), &present(), &wrong_value).is_err());
        let mut extended = present();
        if let Outcome::Present { object } = &mut extended.outcome {
            object.labels.push(Label {
                key: "extra".into(),
                value: "allowed".into(),
            });
        }
        assert!(Intent::prepare(&removal(), &extended, &labels()).is_ok());
    }

    #[test]
    fn prepare_rejects_non_removal_requests() {
        let inspect = Request {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
        };
        assert!(Intent::prepare(&inspect, &present(), &labels()).is_err());
        let capabilities = Request {
            binding: binding(),
            operation: Operation::Capabilities,
        };
        assert!(Intent::prepare(&capabilities, &present(), &labels()).is_err());
    }

    #[test]
    fn binding_conflicts_reject_every_transition_and_restore() {
        let prepared = prepared();
        let unknown = unknown();
        let completed = unknown.reconcile(&removal(), &absent()).unwrap();
        for changed in binding_mutations() {
            let expected = Request {
                binding: changed,
                operation: removal().operation,
            };
            assert!(prepared.begin_dispatch(&expected).is_err());
            assert!(unknown.observe_dispatch(&expected, None).is_err());
            assert!(unknown.reconcile(&expected, &absent()).is_err());
            let bytes = serde_json::to_vec(&completed).unwrap();
            assert!(Intent::restore(&bytes, &expected).is_err());
        }
        for object_id in ["b".repeat(64), "c".repeat(64)] {
            let expected = Request {
                binding: binding(),
                operation: Operation::Remove { object_id },
            };
            assert!(prepared.begin_dispatch(&expected).is_err());
            assert!(unknown.observe_dispatch(&expected, None).is_err());
            assert!(unknown.reconcile(&expected, &absent()).is_err());
            let bytes = serde_json::to_vec(&unknown).unwrap();
            assert!(Intent::restore(&bytes, &expected).is_err());
        }
    }

    #[test]
    fn phase_matrix_rejects_illegal_transitions() {
        let prepared = prepared();
        let unknown = unknown();
        let completed = unknown.reconcile(&removal(), &absent()).unwrap();
        assert!(prepared.begin_dispatch(&removal()).is_ok());
        assert!(prepared.observe_dispatch(&removal(), None).is_err());
        assert!(prepared.reconcile(&removal(), &absent()).is_err());
        assert!(unknown.begin_dispatch(&removal()).is_err());
        assert_eq!(
            unknown.observe_dispatch(&removal(), None).unwrap().phase(),
            Phase::Unknown
        );
        assert_eq!(
            unknown.reconcile(&removal(), &absent()).unwrap().phase(),
            Phase::Completed
        );
        assert!(completed.begin_dispatch(&removal()).is_err());
        assert!(completed.observe_dispatch(&removal(), None).is_err());
        assert!(completed.reconcile(&removal(), &absent()).is_err());
    }

    #[test]
    fn unknown_keeps_unknown_for_none_and_all_mutation_outcomes() {
        let unknown = unknown();
        assert_eq!(
            unknown.observe_dispatch(&removal(), None).unwrap().phase(),
            Phase::Unknown
        );
        for outcome in [
            Outcome::Completed,
            Outcome::RejectedBeforeEffect,
            Outcome::Unknown,
        ] {
            let response = Response {
                binding: binding(),
                operation: removal().operation.clone(),
                outcome,
            };
            let next = unknown
                .observe_dispatch(&removal(), Some(&response))
                .unwrap();
            assert_eq!(next.phase(), Phase::Unknown);
            assert_eq!(next.request(), unknown.request());
            assert_eq!(unknown.phase(), Phase::Unknown);
            assert_eq!(unknown.request(), &removal());
        }
    }

    #[test]
    fn unknown_rejects_invalid_response_without_change() {
        let unknown = unknown();
        let invalid = Response {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
            outcome: Outcome::Absent,
        };
        assert!(unknown
            .observe_dispatch(&removal(), Some(&invalid))
            .is_err());
        assert_eq!(unknown.phase(), Phase::Unknown);
        assert_eq!(unknown.request(), &removal());
    }

    #[test]
    fn completed_requires_exact_id_absence_and_is_terminal() {
        let unknown = unknown();
        for response in [
            present(),
            Response {
                binding: binding(),
                operation: Operation::Inspect { object_id: id() },
                outcome: Outcome::Unavailable,
            },
            Response {
                binding: binding(),
                operation: Operation::Inspect {
                    object_id: "b".repeat(64),
                },
                outcome: Outcome::Absent,
            },
        ] {
            assert!(unknown.reconcile(&removal(), &response).is_err());
        }
        let completed = unknown.reconcile(&removal(), &absent()).unwrap();
        assert_eq!(completed.phase(), Phase::Completed);
        assert!(completed.begin_dispatch(&removal()).is_err());
        assert!(completed.observe_dispatch(&removal(), None).is_err());
        assert!(completed.reconcile(&removal(), &absent()).is_err());
    }

    #[test]
    fn restore_roundtrips_and_enforces_schema_and_phase() {
        for intent in [
            prepared(),
            unknown(),
            unknown().reconcile(&removal(), &absent()).unwrap(),
        ] {
            let bytes = serde_json::to_vec(&intent).unwrap();
            let restored = Intent::restore(&bytes, &removal()).unwrap();
            assert_eq!(restored, intent);
        }
        let mut value = serde_json::to_value(prepared()).unwrap();
        value["schema"] = json!("other/v1");
        assert!(Intent::restore(&serde_json::to_vec(&value).unwrap(), &removal()).is_err());
        value["schema"] = json!("gate-runtime-intent/v1");
        value["phase"] = json!("bogus");
        assert!(Intent::restore(&serde_json::to_vec(&value).unwrap(), &removal()).is_err());
        value["phase"] = json!({"prepared": null});
        assert!(Intent::restore(&serde_json::to_vec(&value).unwrap(), &removal()).is_err());
        value["extra"] = json!(1);
        assert!(Intent::restore(&serde_json::to_vec(&value).unwrap(), &removal()).is_err());
        let legacy = json!({
            "owner_marker": "harness-gate",
            "schema_version": 2,
            "runtime": "docker",
            "runtime_object_id": id()
        });
        assert!(Intent::restore(&serde_json::to_vec(&legacy).unwrap(), &removal()).is_err());
    }

    #[test]
    fn restore_rejects_duplicates_missing_pins_and_bad_bytes() {
        let mut value = serde_json::to_value(prepared()).unwrap();
        value["request"]["binding"]
            .as_object_mut()
            .unwrap()
            .remove("generation");
        assert!(Intent::restore(&serde_json::to_vec(&value).unwrap(), &removal()).is_err());
        let original = serde_json::to_string(&prepared()).unwrap();
        let duplicate = original.replacen(
            r#""phase":"prepared""#,
            r#""phase":"prepared","phase":"prepared""#,
            1,
        );
        assert!(Intent::restore(duplicate.as_bytes(), &removal()).is_err());
        let mut padded = serde_json::to_vec(&prepared()).unwrap();
        padded.extend(std::iter::repeat_n(b' ', crate::MAX_MESSAGE_BYTES));
        assert!(Intent::restore(&padded, &removal()).is_err());
        let mut invalid = serde_json::to_value(prepared()).unwrap();
        invalid["request"]["binding"]["invocation"] = json!("SECRET-ECHO\n");
        let bytes = serde_json::to_vec(&invalid).unwrap();
        let error = Intent::restore(&bytes, &removal()).unwrap_err();
        assert!(!error.to_string().contains("SECRET-ECHO"));
        let before = bytes.clone();
        let _ = Intent::restore(&bytes, &removal());
        assert_eq!(bytes, before);
    }
}
