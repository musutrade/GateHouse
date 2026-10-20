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
