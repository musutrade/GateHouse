//! Core-owned request minting from already protected installation/config bytes.
use gatehouse_runtime_protocol::v3::{
    self, Binding, Engine, Operation, Protocol, Request, Target, Transport,
};
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
type Result<T> = std::result::Result<T, &'static str>;
fn hash(domain: &str, fields: &[&str]) -> String {
    let mut hash = Sha256::new();
    for field in std::iter::once(domain).chain(fields.iter().copied()) {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
#[cfg(target_os = "linux")]
fn nonce() -> Result<[u8; 16]> {
    let mut bytes = [0u8; 16];
    let mut offset = 0;
    while offset < bytes.len() {
        let n = unsafe {
            libc::getrandom(bytes[offset..].as_mut_ptr().cast(), bytes.len() - offset, 0)
        };
        if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if n <= 0 {
            return Err("runtime entropy unavailable");
        }
        offset += n as usize;
    }
    Ok(bytes)
}
#[cfg(not(any(target_os = "linux")))]
fn nonce() -> Result<[u8; 16]> {
    Err("v3 runtime requires Linux")
}

pub(super) fn mint(
    config: &[u8],
    adapter_sha256: &str,
    adapter_version: &str,
    image: &str,
) -> Result<Request> {
    mint_with(config, adapter_sha256, adapter_version, image, nonce)
}
fn mint_with(
    config: &[u8],
    adapter_sha256: &str,
    adapter_version: &str,
    image: &str,
    entropy: impl FnOnce() -> Result<[u8; 16]>,
) -> Result<Request> {
    if config.len() > 65_536 {
        return Err("runtime config too large");
    }
    v3::validate_image(image).map_err(|_| "invalid readiness image")?;
    let c: Config = serde_json::from_slice(config).map_err(|_| "invalid runtime config")?;
    if c.schema != "gate-runtime-engine-config/v3" {
        return Err("invalid runtime config schema");
    }
    let config_hash = format!("{:x}", Sha256::digest(config));
    // Exact config digest commits all engine identity fields and byte spelling.
    let generation = hash(
        "gate-runtime/v3/generation/1",
        &[adapter_sha256, adapter_version, &config_hash, image],
    );
    let invocation = entropy()?
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let operation_key = hash(
        "gate-runtime/v3/operation/1",
        &[&invocation, &generation, &config_hash, image],
    );
    let request = Request {
        binding: Binding {
            protocol: Protocol::V3,
            adapter_sha256: adapter_sha256.into(),
            adapter_version: adapter_version.into(),
            invocation,
            operation_key,
            generation,
            adapter_config_sha256: config_hash,
            engine: c.engine,
            transport: c.transport,
            socket_path: c.socket_path,
            socket_uid: c.socket_uid,
            principal_uid: c.principal_uid,
            rootless: c.rootless,
            api_version: c.api_version,
            server_version: c.server_version,
            daemon_id: c.daemon_id,
            scope: c.scope,
        },
        operation: Operation {
            readiness: Target {
                image: image.into(),
            },
        },
    };
    request
        .validate()
        .map_err(|_| "invalid runtime readiness pins")?;
    Ok(request)
}

pub(super) fn validate(config: &[u8], expected: &Request) -> Result<()> {
    expected
        .validate()
        .map_err(|_| "invalid runtime readiness pins")?;
    let mut nonce = [0u8; 16];
    for (i, byte) in nonce.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&expected.binding.invocation[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid invocation")?;
    }
    let rebuilt = mint_with(
        config,
        &expected.binding.adapter_sha256,
        &expected.binding.adapter_version,
        &expected.operation.readiness.image,
        || Ok(nonce),
    )?;
    if rebuilt != *expected {
        return Err("runtime v3 config binding mismatch");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Vec<u8> {
        br#"{"schema":"gate-runtime-engine-config/v3","engine":"docker","transport":"unix-http","socket_path":"/run/fixture.sock","socket_uid":1000,"principal_uid":1000,"rootless":false,"api_version":"1.47","server_version":"27","daemon_id":"fixture","scope":"test"}"#.to_vec()
    }
    #[test]
    fn keys_bind_exact_snapshot_adapter_image_and_nonce() {
        let c = config();
        let a = "a".repeat(64);
        let first = mint_with(&c, &a, "0.2.0", "image:tag", || Ok([1; 16])).unwrap();
        let second = mint_with(&c, &a, "0.2.0", "image:tag", || Ok([2; 16])).unwrap();
        assert_ne!(first.binding.invocation, second.binding.invocation);
        assert_ne!(first.binding.operation_key, second.binding.operation_key);
        assert_eq!(first.binding.generation, second.binding.generation);
        for (bytes, adapter, image) in [
            (config(), "b".repeat(64), "image:tag"),
            ([c.as_slice(), b" "].concat(), a.clone(), "image:tag"),
            (config(), a, "image:other"),
        ] {
            let changed = mint_with(&bytes, &adapter, "0.2.0", image, || Ok([1; 16])).unwrap();
            assert_ne!(first.binding.generation, changed.binding.generation);
            assert_ne!(first.binding.operation_key, changed.binding.operation_key);
        }
    }
    #[test]
    fn entropy_failure_has_no_template_fallback() {
        assert!(
            mint_with(&config(), &"a".repeat(64), "0.2.0", "image", || Err(
                "runtime entropy unavailable"
            ))
            .is_err()
        );
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn production_nonce_is_fresh() {
        let a = mint(&config(), &"a".repeat(64), "0.2.0", "image").unwrap();
        let b = mint(&config(), &"a".repeat(64), "0.2.0", "image").unwrap();
        assert_ne!(a.binding.invocation, b.binding.invocation);
    }
}
