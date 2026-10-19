//! Data validation only. No process execution, artifact verification, lease
//! persistence, ambient context discovery, or authority to delete a resource.
mod wire;

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_MESSAGE_BYTES: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub enum Protocol {
    #[serde(rename = "gate-runtime/v1")]
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(try_from = "String")]
pub enum Engine {
    Docker,
    Podman,
}

/// Explicit trusted configuration, never filled from the current environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Namespace {
    pub engine: Engine,
    pub endpoint: String,
    pub context: String,
    pub principal: String,
    pub rootless: bool,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub protocol: Protocol,
    pub adapter_sha256: String,
    pub adapter_version: String,
    pub invocation: String,
    pub operation_key: String,
    pub generation: String,
    pub namespace: Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Capabilities,
    Inspect { object_id: String },
    Remove { object_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub binding: Binding,
    pub operation: Operation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub object_id: String,
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(try_from = "String")]
pub enum Capability {
    Inspect,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Capabilities { supported: Vec<Capability> },
    Present { object: Object },
    Absent,
    Unavailable,
    Completed,
    RejectedBeforeEffect,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub binding: Binding,
    pub operation: Operation,
    pub outcome: Outcome,
}

/// Deliberately excludes untrusted input values from diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid(pub &'static str);
impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Invalid {}
type Result<T> = std::result::Result<T, Invalid>;

fn nonempty(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(Invalid("missing or invalid identity"));
    }
    Ok(())
}

fn digest(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Invalid("expected full lowercase hexadecimal identity"));
    }
    Ok(())
}

impl Binding {
    pub fn validate(&self) -> Result<()> {
        digest(&self.adapter_sha256)?;
        for value in [
            &self.adapter_version,
            &self.invocation,
            &self.operation_key,
            &self.generation,
            &self.namespace.endpoint,
            &self.namespace.context,
            &self.namespace.principal,
            &self.namespace.scope,
        ] {
            nonempty(value)?;
        }
        Ok(())
    }
}

impl Request {
    pub fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        match &self.operation {
            Operation::Capabilities => Ok(()),
            Operation::Inspect { object_id } | Operation::Remove { object_id } => digest(object_id),
        }
    }

    /// Reject operation-key reuse with changed content, including generation.
    pub fn require_same_intent(&self, previous: &Self) -> Result<()> {
        self.validate()?;
        previous.validate()?;
        if self != previous {
            return Err(Invalid("request differs from recorded intent"));
        }
        Ok(())
    }
}

fn parse<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Invalid("message exceeds protocol limit"));
    }
    serde_json::from_slice(bytes).map_err(|_| Invalid("invalid protocol message"))
}

pub fn parse_request(bytes: &[u8]) -> Result<Request> {
    let request: Request = parse(bytes)?;
    request.validate()?;
    Ok(request)
}

pub fn parse_response(bytes: &[u8], expected: &Request) -> Result<Response> {
    let response: Response = parse(bytes)?;
    response.validate(expected)?;
    Ok(response)
}

fn validate_labels(labels: &[Label]) -> Result<()> {
    let mut keys = BTreeSet::new();
    if labels.is_empty() {
        return Err(Invalid("missing ownership labels"));
    }
    for label in labels {
        nonempty(&label.key)?;
        nonempty(&label.value)?;
        if !keys.insert(&label.key) {
            return Err(Invalid("duplicate ownership label"));
        }
    }
    Ok(())
}

impl Response {
    pub fn validate(&self, expected: &Request) -> Result<()> {
        expected.validate()?;
        if self.binding != expected.binding || self.operation != expected.operation {
            return Err(Invalid("response binding mismatch"));
        }
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

    /// Core supplies the required ownership labels from its trusted lease.
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

/// An adapter's mutation completion claim alone cannot clear an intent. This
/// checks a separately obtained exact-ID inspection; persistence and locking
/// remain the caller's responsibility, as does authenticating the adapter.
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
