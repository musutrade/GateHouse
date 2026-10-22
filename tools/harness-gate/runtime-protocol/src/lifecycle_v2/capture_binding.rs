//! Frozen capture operation-key binding. No dispatch, freshness or authority.
use serde::Serialize;
use sha2::{Digest, Sha256};
#[derive(Serialize)]
struct Envelope<'a> {
    domain: &'static str,
    request_sha256: &'a str,
    host_sha256: &'a str,
    installation_sha256: &'a str,
    config_sha256: &'a str,
    executable_sha256: &'a str,
}
/// SHA256 of the explicit field-order envelope. The request digest must cover
/// the canonical full request including invocation; this function does not prove
/// that a caller actually obtained any digest from trusted bytes.
pub fn operation_key(
    request: &str,
    host: &str,
    installation: &str,
    config: &str,
    executable: &str,
) -> Result<String, &'static str> {
    let bytes = envelope(request, host, installation, config, executable)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn envelope(
    request: &str,
    host: &str,
    installation: &str,
    config: &str,
    executable: &str,
) -> Result<Vec<u8>, &'static str> {
    for hash in [request, host, installation, config, executable] {
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("capture binding digest rejected");
        }
    }
    serde_json::to_vec(&Envelope {
        domain: "gate-runtime/podman-capture-key-v1",
        request_sha256: request,
        host_sha256: host,
        installation_sha256: installation,
        config_sha256: config,
        executable_sha256: executable,
    })
    .map_err(|_| "binding encoding")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn golden_key_and_all_snapshot_changes() {
        let hashes: Vec<String> = (0..5).map(|i| format!("{i}").repeat(64)).collect();
        let key =
            operation_key(&hashes[0], &hashes[1], &hashes[2], &hashes[3], &hashes[4]).unwrap();
        assert_eq!(
            key,
            "5071ab535b718e57e663e2de6da7fdece81bab385f413773a00aa7c537d5de90"
        );
        for i in 0..5 {
            let mut changed = hashes.clone();
            changed[i] = "f".repeat(64);
            assert_ne!(
                key,
                operation_key(
                    &changed[0],
                    &changed[1],
                    &changed[2],
                    &changed[3],
                    &changed[4]
                )
                .unwrap()
            );
        }
        assert!(operation_key("bad", &hashes[1], &hashes[2], &hashes[3], &hashes[4]).is_err());
    }
}
