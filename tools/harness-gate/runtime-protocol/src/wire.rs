//! Preserve one encoding for each variant without first buffering into Value,
//! which would discard duplicate object keys before typed validation.
use crate::{Capability, Engine, Object, Operation, Outcome, Protocol};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    object_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Present {
    object: Object,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capabilities {
    supported: Vec<Capability>,
}

fn tag<'de, M: MapAccess<'de>>(map: &mut M) -> Result<String, M::Error> {
    map.next_key()?
        .ok_or_else(|| de::Error::custom("missing variant"))
}

fn end<'de, M: MapAccess<'de>>(map: &mut M) -> Result<(), M::Error> {
    if map.next_key::<de::IgnoredAny>()?.is_some() {
        return Err(de::Error::custom("multiple variant fields"));
    }
    Ok(())
}

struct OperationVisitor;
impl<'de> Visitor<'de> for OperationVisitor {
    type Value = Operation;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string unit operation or one tagged operation object")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Operation, E> {
        match value {
            "capabilities" => Ok(Operation::Capabilities),
            _ => Err(E::custom("invalid string operation")),
        }
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Operation, M::Error> {
        let operation = match tag(&mut map)?.as_str() {
            "inspect" => Operation::Inspect {
                object_id: map.next_value::<Target>()?.object_id,
            },
            "remove" => Operation::Remove {
                object_id: map.next_value::<Target>()?.object_id,
            },
            _ => return Err(de::Error::custom("invalid object operation")),
        };
        end(&mut map)?;
        Ok(operation)
    }
}

impl<'de> Deserialize<'de> for Operation {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OperationVisitor)
    }
}

struct OutcomeVisitor;
impl<'de> Visitor<'de> for OutcomeVisitor {
    type Value = Outcome;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string unit outcome or one tagged outcome object")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Outcome, E> {
        match value {
            "absent" => Ok(Outcome::Absent),
            "unavailable" => Ok(Outcome::Unavailable),
            "completed" => Ok(Outcome::Completed),
            "rejected-before-effect" => Ok(Outcome::RejectedBeforeEffect),
            "unknown" => Ok(Outcome::Unknown),
            _ => Err(E::custom("invalid string outcome")),
        }
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Outcome, M::Error> {
        let outcome = match tag(&mut map)?.as_str() {
            "present" => Outcome::Present {
                object: map.next_value::<Present>()?.object,
            },
            "capabilities" => Outcome::Capabilities {
                supported: map.next_value::<Capabilities>()?.supported,
            },
            _ => return Err(de::Error::custom("invalid object outcome")),
        };
        end(&mut map)?;
        Ok(outcome)
    }
}

impl<'de> Deserialize<'de> for Outcome {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OutcomeVisitor)
    }
}

impl TryFrom<String> for Protocol {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "gate-runtime/v1" => Ok(Self::V1),
            _ => Err("unsupported protocol"),
        }
    }
}

impl TryFrom<String> for Engine {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "docker" => Ok(Self::Docker),
            "podman" => Ok(Self::Podman),
            _ => Err("unsupported engine"),
        }
    }
}

impl TryFrom<String> for Capability {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "inspect" => Ok(Self::Inspect),
            "remove" => Ok(Self::Remove),
            _ => Err("unsupported capability"),
        }
    }
}
