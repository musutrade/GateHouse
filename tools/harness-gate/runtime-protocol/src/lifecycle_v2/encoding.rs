//! Bounded canonical identity envelope; not a journal or filesystem verifier.
use super::profile::SlotIdentity;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA: &str = "gate-runtime/podman-slot-identity-v1";
const LIMIT: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    identity: SlotIdentity,
}

/// UTF-8 JSON with lexicographically ordered object keys, no whitespace/newline.
/// This envelope is distinct from every historical lifecycle slot format.
pub fn encode(identity: &SlotIdentity) -> Result<Vec<u8>, &'static str> {
    identity.validate()?;
    // Struct fields below explicitly fix lexicographic order at every level.
    // No serde_json::Value/Map is involved, including under preserve_order.
    #[derive(Serialize)]
    struct CanonicalProfile<'a> {
        engine: &'a str,
        product: &'a str,
        protocol: &'a str,
        version: &'a str,
    }
    #[derive(Serialize)]
    struct CanonicalIdentity<'a> {
        config_sha256: &'a str,
        executable_sha256: &'a str,
        installation_sha256: &'a str,
        profile: CanonicalProfile<'a>,
    }
    #[derive(Serialize)]
    struct CanonicalEnvelope<'a> {
        identity: CanonicalIdentity<'a>,
        schema: &'a str,
    }
    let value = CanonicalEnvelope {
        identity: CanonicalIdentity {
            config_sha256: &identity.config_sha256,
            executable_sha256: &identity.executable_sha256,
            installation_sha256: &identity.installation_sha256,
            profile: CanonicalProfile {
                engine: &identity.profile.engine,
                product: &identity.profile.product,
                protocol: &identity.profile.protocol,
                version: &identity.profile.version,
            },
        },
        schema: SCHEMA,
    };
    let bytes = serde_json::to_vec(&value).map_err(|_| "invalid Podman slot encoding")?;
    if bytes.len() > LIMIT {
        return Err("Podman slot identity exceeds limit");
    }
    Ok(bytes)
}

/// Deserialize directly into structs before canonical comparison: duplicate and
/// unknown fields must fail rather than being collapsed into a JSON map.
pub fn decode(bytes: &[u8]) -> Result<SlotIdentity, &'static str> {
    if bytes.len() > LIMIT {
        return Err("Podman slot identity exceeds limit");
    }
    let value: Envelope =
        serde_json::from_slice(bytes).map_err(|_| "invalid Podman slot envelope")?;
    if value.schema != SCHEMA || encode(&value.identity)? != bytes {
        return Err("noncanonical or wrong-schema Podman slot identity");
    }
    Ok(value.identity)
}

/// Match exact snapshots supplied by the caller; never hash reserialized JSON.
/// This does NOT validate descriptor schemas, provenance, permissions, stable
/// file handles or engine identity. Core must establish those before use.
pub fn match_snapshots(
    identity: &SlotIdentity,
    installation: &[u8],
    config: &[u8],
    executable: &[u8],
) -> Result<(), &'static str> {
    identity.validate()?;
    for (bytes, expected) in [
        (installation, &identity.installation_sha256),
        (config, &identity.config_sha256),
        (executable, &identity.executable_sha256),
    ] {
        if format!("{:x}", Sha256::digest(bytes)) != *expected {
            return Err("Podman slot snapshot mismatch");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle_v2::profile::{Profile, PRODUCT, PROTOCOL, VERSION};

    fn identity() -> SlotIdentity {
        SlotIdentity {
            profile: Profile {
                protocol: PROTOCOL.into(),
                product: PRODUCT.into(),
                version: VERSION.into(),
                engine: "podman".into(),
            },
            installation_sha256: format!("{:x}", Sha256::digest(b"installation")),
            config_sha256: format!("{:x}", Sha256::digest(b"config")),
            executable_sha256: format!("{:x}", Sha256::digest(b"executable")),
        }
    }

    #[test]
    fn canonical_envelope_has_stable_order_and_schema() {
        let original = identity();
        let bytes = encode(&original).unwrap();
        assert_eq!(decode(&bytes).unwrap(), original);
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.starts_with("{\"identity\":{\"config_sha256\":"));
        assert!(text.ends_with(&format!(",\"schema\":\"{SCHEMA}\"}}")));
        assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);
    }

    #[test]
    fn whitespace_trailing_and_reordered_encoding_are_rejected() {
        let bytes = encode(&identity()).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let reordered = format!(
            "{{\"schema\":\"{SCHEMA}\",\"identity\":{}}}",
            value["identity"]
        );
        for invalid in [
            serde_json::to_vec_pretty(&value).unwrap(),
            [bytes.clone(), b"\n".to_vec()].concat(),
            [bytes, b"{}".to_vec()].concat(),
            reordered.into_bytes(),
        ] {
            assert!(decode(&invalid).is_err());
        }
    }

    #[test]
    fn duplicate_fields_at_each_level_are_rejected() {
        let text = String::from_utf8(encode(&identity()).unwrap()).unwrap();
        for invalid in [
            text.replacen("{", &format!("{{\"schema\":\"{SCHEMA}\","), 1),
            text.replace(
                "\"identity\":{",
                "\"identity\":{\"config_sha256\":\"duplicate\",",
            ),
            text.replace("\"profile\":{", "\"profile\":{\"engine\":\"podman\","),
        ] {
            assert!(decode(invalid.as_bytes()).is_err());
        }
    }

    #[test]
    fn every_required_field_is_required() {
        let value: serde_json::Value =
            serde_json::from_slice(&encode(&identity()).unwrap()).unwrap();
        for path in [vec![], vec!["identity"], vec!["identity", "profile"]] {
            let mut object = &value;
            for key in &path {
                object = &object[*key];
            }
            for field in object.as_object().unwrap().keys() {
                let mut changed = value.clone();
                let mut target = &mut changed;
                for key in &path {
                    target = &mut target[*key];
                }
                target.as_object_mut().unwrap().remove(field);
                assert!(decode(&serde_json::to_vec(&changed).unwrap()).is_err());
            }
        }
    }

    #[test]
    fn nested_extra_field_and_old_schema_are_rejected() {
        let value: serde_json::Value =
            serde_json::from_slice(&encode(&identity()).unwrap()).unwrap();
        let mut extra = value.clone();
        extra["identity"]["profile"]["daemon_id"] = "docker-id".into();
        let mut old = value;
        old["schema"] = "gate-runtime/lifecycle-slot-v1".into();
        for invalid in [extra, old] {
            assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
    }

    #[test]
    fn extra_envelope_and_identity_fields_are_rejected() {
        let value: serde_json::Value =
            serde_json::from_slice(&encode(&identity()).unwrap()).unwrap();
        let mut envelope = value.clone();
        envelope["authority"] = "adopt".into();
        let mut identity = value;
        identity["identity"]["authority"] = "adopt".into();
        for invalid in [envelope, identity] {
            assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
    }

    #[test]
    fn full_canonical_bytes_are_feature_independent() {
        let id = identity();
        let expected = format!(
            "{{\"identity\":{{\"config_sha256\":\"{}\",\"executable_sha256\":\"{}\",\"installation_sha256\":\"{}\",\"profile\":{{\"engine\":\"podman\",\"product\":\"podman-lifecycle\",\"protocol\":\"gate-runtime/lifecycle-v2\",\"version\":\"0.1.0\"}}}},\"schema\":\"gate-runtime/podman-slot-identity-v1\"}}",
            id.config_sha256, id.executable_sha256, id.installation_sha256
        );
        assert_eq!(encode(&id).unwrap(), expected.as_bytes());
    }

    #[test]
    fn oversized_and_invalid_utf8_are_rejected() {
        assert!(decode(&vec![b' '; LIMIT + 1]).is_err());
        assert!(decode(&[0xff]).is_err());
    }

    #[test]
    fn exact_bytes_match_but_each_replaced_snapshot_fails() {
        let id = identity();
        match_snapshots(&id, b"installation", b"config", b"executable").unwrap();
        for values in [
            [b"installation ".as_slice(), b"config", b"executable"],
            [b"installation", b"config ", b"executable"],
            [b"installation", b"config", b"executable "],
        ] {
            assert!(match_snapshots(&id, values[0], values[1], values[2]).is_err());
        }
    }
}
