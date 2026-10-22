//! Protected installation snapshots. No process execution, daemon access or
//! authority minting. Callers must keep/revalidate held descriptors through spawn.
use super::{
    canonical,
    wire::{EngineBinding, Request, PROTOCOL},
};
use crate::{digest, Invalid, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    schema: String,
    installation_path: String,
    installation_sha256: String,
    config_path: String,
    config_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Installation {
    schema: String,
    product: String,
    version: String,
    protocol: String,
    executable_path: String,
    executable_sha256: String,
    config_schema: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    engine_binding: EngineBinding,
}

fn parse<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    let value = canonical::parse_canonical(bytes)?;
    let typed: T = serde_json::from_value(value)
        .map_err(|_| Invalid("invalid lifecycle installation schema"))?;
    if canonical::encode(
        &serde_json::to_value(&typed).map_err(|_| Invalid("invalid installation encoding"))?,
    )? != bytes
    {
        return Err(Invalid("installation fields changed during parsing"));
    }
    Ok(typed)
}
#[cfg(unix)]
mod files;
#[cfg(unix)]
pub use files::VerifiedInstallation;

#[cfg(target_os = "linux")]
mod sealed;
#[cfg(target_os = "linux")]
pub use sealed::parse_protected_inputs;
