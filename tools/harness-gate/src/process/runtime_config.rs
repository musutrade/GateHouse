//! Pure v2 config-byte binding. No file/socket lookup, process identity check,
//! daemon authentication or spawning. Callers supply an independently trusted
//! expected request; successful equality is not installed-artifact authority.
use gatehouse_runtime_protocol::v2::{Engine, Request, Transport};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    engine: Engine,
    transport: Transport,
    socket_path: String,
    socket_uid: u32,
    principal_uid: u32,
    rootless: bool,
    api_version: String,
    server_version: String,
    daemon_id: String,
    scope: String,
}

/// Static errors never include supplied bytes, identities or parser diagnostics.
/// The raw bytes (including whitespace) are hashed without reserialization.
/// Rust's strict u32/Unicode parser intentionally rejects JSON -0 and lone
/// surrogates even where the Python fixture parser can accept them.
#[allow(dead_code)]
pub(super) fn validate_config_binding(
    bytes: &[u8],
    expected: &Request,
) -> Result<(), &'static str> {
    if bytes.len() > 65_536 {
        return Err("runtime config too large");
    }
    expected.validate().map_err(|_| "invalid runtime request")?;
    let config: Config = serde_json::from_slice(bytes).map_err(|_| "invalid runtime config")?;
    if config.schema != "gate-runtime-engine-config/v2" {
        return Err("invalid runtime config schema");
    }
    let mut binding = expected.binding.clone();
    binding.adapter_config_sha256 = format!("{:x}", Sha256::digest(bytes));
    binding.engine = config.engine;
    binding.transport = config.transport;
    binding.socket_path = config.socket_path;
    binding.socket_uid = config.socket_uid;
    binding.principal_uid = config.principal_uid;
    binding.rootless = config.rootless;
    binding.api_version = config.api_version;
    binding.server_version = config.server_version;
    binding.daemon_id = config.daemon_id;
    binding.scope = config.scope;
    binding
        .validate()
        .map_err(|_| "invalid runtime config pins")?;
    if binding != expected.binding {
        return Err("runtime config binding mismatch");
    }
    Ok(())
}
