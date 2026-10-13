//! Generic evidence, policy and reporting semantics shipped with Harness-Gate.
//! The replay comparator remains non-authoritative migration tooling.

mod comparison;
pub mod cross_component;
pub mod evidence;
mod json;
pub mod model;
pub mod policy;
pub mod project;
pub mod project_report;
pub mod ratchet;
pub mod replay;
mod schema;

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub class: ReasonClass,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonClass {
    ModelError,
    MeasurementError,
}

pub type Result<T> = std::result::Result<T, Error>;

fn error(message: impl Into<String>) -> Error {
    Error {
        class: ReasonClass::MeasurementError,
        message: message.into(),
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(error(message))
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn array(value: &Value) -> &[Value] {
    value.as_array().expect("schema-validated array")
}

fn string(value: &Value) -> &str {
    value.as_str().expect("schema-validated string")
}

fn index<'a>(records: &'a Value, key: &str, label: &str) -> Result<BTreeMap<&'a str, &'a Value>> {
    let mut result = BTreeMap::new();
    for record in array(records) {
        let id = string(&record[key]);
        require(
            result.insert(id, record).is_none(),
            format!("duplicate {label}: {id}"),
        )?;
    }
    Ok(result)
}

pub use json::{canonical, parse};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_tests;
