//! Explicit profile selection and durable slot identity, independent of engine I/O.
use serde::{Deserialize, Serialize};

pub const PROTOCOL: &str = "gate-runtime/lifecycle-v2";
pub const PRODUCT: &str = "podman-lifecycle";
pub const VERSION: &str = "0.1.0";

/// Exact operator-selected product tuple. No inference from executable names,
/// environment, endpoint responses, or another protocol's engine field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub protocol: String,
    pub product: String,
    pub version: String,
    pub engine: String,
}

impl Profile {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.protocol != PROTOCOL
            || self.product != PRODUCT
            || self.version != VERSION
            || self.engine != "podman"
        {
            return Err("unsupported Podman lifecycle profile");
        }
        Ok(())
    }
}

/// Proposed durable identity for new slots. This is not a Docker journal reader.
/// Missing identity, including historical slots, must not become a default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotIdentity {
    pub profile: Profile,
    pub installation_sha256: String,
    pub config_sha256: String,
    pub executable_sha256: String,
}

impl SlotIdentity {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.profile.validate()?;
        for hash in [
            &self.installation_sha256,
            &self.config_sha256,
            &self.executable_sha256,
        ] {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("invalid Podman slot identity digest");
            }
        }
        Ok(())
    }

    /// A mismatch requires original-profile recovery, never adoption or rewriting.
    /// Matching bytes alone do not grant lease or operator cleanup authority.
    pub fn require_same(&self, recorded: &Self) -> Result<(), &'static str> {
        self.validate()?;
        recorded.validate()?;
        if self != recorded {
            return Err("Podman slot profile or installation changed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> SlotIdentity {
        SlotIdentity {
            profile: Profile {
                protocol: PROTOCOL.into(),
                product: PRODUCT.into(),
                version: VERSION.into(),
                engine: "podman".into(),
            },
            installation_sha256: "a".repeat(64),
            config_sha256: "b".repeat(64),
            executable_sha256: "c".repeat(64),
        }
    }

    #[test]
    fn exact_profile_and_identity_roundtrip() {
        let original = identity();
        let decoded: SlotIdentity =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        original.require_same(&decoded).unwrap();
    }

    #[test]
    fn docker_and_mixed_profiles_are_rejected() {
        for field in ["protocol", "product", "version", "engine"] {
            let mut value = serde_json::to_value(identity()).unwrap();
            value["profile"][field] = serde_json::json!(match field {
                "protocol" => "gate-runtime/lifecycle-v1",
                "product" => "docker-lifecycle",
                "version" => "0.2.0",
                _ => "docker",
            });
            let changed: SlotIdentity = serde_json::from_value(value).unwrap();
            assert!(changed.validate().is_err(), "{field}");
        }
    }

    #[test]
    fn changed_installation_config_or_executable_cannot_adopt_slot() {
        for field in ["installation_sha256", "config_sha256", "executable_sha256"] {
            let original = identity();
            let mut value = serde_json::to_value(&original).unwrap();
            value[field] = serde_json::json!("d".repeat(64));
            let changed: SlotIdentity = serde_json::from_value(value).unwrap();
            assert!(original.require_same(&changed).is_err());
        }
    }

    #[test]
    fn missing_profile_and_unknown_authority_fields_are_rejected() {
        for value in [serde_json::json!({"released":true}), {
            let mut value = serde_json::to_value(identity()).unwrap();
            value["authority"] = serde_json::json!("adopt-docker");
            value
        }] {
            assert!(serde_json::from_value::<SlotIdentity>(value).is_err());
        }
    }

    #[test]
    fn noncanonical_hashes_are_rejected() {
        for hash in ["A".repeat(64), "a".repeat(63), "g".repeat(64)] {
            let mut value = identity();
            value.config_sha256 = hash;
            assert!(value.validate().is_err());
        }
    }
}
