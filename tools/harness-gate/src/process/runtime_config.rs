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

#[cfg(test)]
mod tests {
    use super::*;
    use gatehouse_runtime_protocol::v2::{Binding, Protocol, Request};
    use gatehouse_runtime_protocol::Operation;
    use serde_json::{json, Value};

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn baseline() -> Value {
        json!({
            "schema": "gate-runtime-engine-config/v2",
            "engine": "docker",
            "transport": "unix-http",
            "socket_path": "/run/docker.sock",
            "socket_uid": 1000,
            "principal_uid": 1000,
            "rootless": false,
            "api_version": "1.47",
            "server_version": "27.0.1",
            "daemon_id": "daemon-1",
            "scope": "scope-1"
        })
    }

    fn bytes(value: &Value) -> Vec<u8> {
        serde_json::to_vec(value).expect("json")
    }

    fn expected_for(config: &[u8]) -> Request {
        Request {
            binding: Binding {
                protocol: Protocol::V2,
                adapter_sha256: "a".repeat(64),
                adapter_version: "1.0.0".into(),
                invocation: "inv-1".into(),
                operation_key: "op-1".into(),
                generation: "gen-1".into(),
                adapter_config_sha256: digest(config),
                engine: Engine::Docker,
                transport: Transport::UnixHttp,
                socket_path: "/run/docker.sock".into(),
                socket_uid: 1000,
                principal_uid: 1000,
                rootless: false,
                api_version: "1.47".into(),
                server_version: "27.0.1".into(),
                daemon_id: "daemon-1".into(),
                scope: "scope-1".into(),
            },
            operation: Operation::Capabilities,
        }
    }

    fn validate(config: &Value) -> Result<(), &'static str> {
        let raw = bytes(config);
        validate_config_binding(&raw, &expected_for(&raw))
    }

    #[test]
    fn accepts_exact_baseline_and_hashes_original_bytes() {
        let raw = bytes(&baseline());
        assert_eq!(validate_config_binding(&raw, &expected_for(&raw)), Ok(()));

        let pretty = serde_json::to_vec_pretty(&baseline()).unwrap();
        assert_ne!(raw, pretty);
        let expected = expected_for(&raw);
        assert_eq!(
            validate_config_binding(&pretty, &expected),
            Err("runtime config binding mismatch")
        );
        assert_eq!(
            validate_config_binding(&pretty, &expected_for(&pretty)),
            Ok(())
        );
    }

    #[test]
    fn rejects_missing_duplicate_unknown_trailing_and_invalid_utf8() {
        let mut missing = baseline();
        missing.as_object_mut().unwrap().remove("scope");
        assert_eq!(validate(&missing), Err("invalid runtime config"));

        let raw = bytes(&baseline());
        let duplicate = format!(
            "{{\"schema\":\"gate-runtime-engine-config/v2\",{}",
            &String::from_utf8(raw).unwrap()[1..]
        );
        assert_eq!(
            validate_config_binding(duplicate.as_bytes(), &expected_for(duplicate.as_bytes())),
            Err("invalid runtime config")
        );

        let mut unknown = baseline();
        unknown["extra"] = json!(1);
        assert_eq!(validate(&unknown), Err("invalid runtime config"));

        let mut trailing = bytes(&baseline());
        trailing.extend_from_slice(b"{}");
        assert_eq!(
            validate_config_binding(&trailing, &expected_for(&trailing)),
            Err("invalid runtime config")
        );

        let invalid_utf8 = vec![b'{', 0xff, b'}'];
        assert_eq!(
            validate_config_binding(&invalid_utf8, &expected_for(&invalid_utf8)),
            Err("invalid runtime config")
        );

        let raw = String::from_utf8(bytes(&baseline())).unwrap();
        let surrogate = raw.replace("\"scope\":\"scope-1\"", "\"scope\":\"\\ud800\"");
        assert_eq!(
            validate_config_binding(surrogate.as_bytes(), &expected_for(surrogate.as_bytes())),
            Err("invalid runtime config")
        );
    }

    #[test]
    fn rejects_numeric_spellings_and_bounds() {
        let raw = String::from_utf8(bytes(&baseline())).unwrap();
        for (old, new) in [
            ("\"socket_uid\":1000", "\"socket_uid\":-0"),
            ("\"socket_uid\":1000", "\"socket_uid\":1000.0"),
            ("\"socket_uid\":1000", "\"socket_uid\":true"),
            ("\"socket_uid\":1000", "\"socket_uid\":-1"),
            ("\"principal_uid\":1000", "\"principal_uid\":4294967296"),
            ("\"rootless\":false", "\"rootless\":0"),
        ] {
            let changed = raw.replace(old, new);
            assert_eq!(
                validate_config_binding(changed.as_bytes(), &expected_for(changed.as_bytes())),
                Err("invalid runtime config"),
                "{new}"
            );
        }
    }

    #[test]
    fn rejects_schema_engine_and_transport() {
        let mut schema = baseline();
        schema["schema"] = json!("other");
        assert_eq!(validate(&schema), Err("invalid runtime config schema"));

        for (field, value) in [("engine", "podman"), ("transport", "tcp")] {
            let mut changed = baseline();
            changed[field] = json!(value);
            assert_eq!(validate(&changed), Err("invalid runtime config"), "{field}");
        }
    }

    #[test]
    fn rejects_lexical_path_api_and_control_text() {
        for (field, value) in [
            ("socket_path", "run/docker.sock"),
            ("socket_path", "/run/../docker.sock"),
            ("socket_path", "/run//docker.sock"),
            ("api_version", "1"),
            ("api_version", "1.048"),
            ("scope", "scope\u{0}"),
            ("scope", "scope\u{85}"),
        ] {
            let mut changed = baseline();
            changed[field] = json!(value);
            assert_eq!(
                validate(&changed),
                Err("invalid runtime config pins"),
                "{field}"
            );
        }
    }

    #[test]
    fn every_config_derived_binding_pin_mismatch_is_rejected() {
        for (field, value) in [
            ("socket_path", json!("/run/other.sock")),
            ("socket_uid", json!(1001)),
            ("principal_uid", json!(1001)),
            ("rootless", json!(true)),
            ("api_version", json!("1.48")),
            ("server_version", json!("27.9.9")),
            ("daemon_id", json!("daemon-2")),
            ("scope", json!("scope-2")),
        ] {
            let mut changed = baseline();
            changed[field] = value;
            assert_eq!(
                validate(&changed),
                Err("runtime config binding mismatch"),
                "{field}"
            );
        }
    }

    #[test]
    fn digest_mismatch_and_size_boundary_are_static() {
        let raw = bytes(&baseline());
        let mut expected = expected_for(&raw);
        expected.binding.adapter_config_sha256 = "b".repeat(64);
        assert_eq!(
            validate_config_binding(&raw, &expected),
            Err("runtime config binding mismatch")
        );

        let too_large = vec![b' '; 65_537];
        assert_eq!(
            validate_config_binding(&too_large, &expected_for(&too_large)),
            Err("runtime config too large")
        );
        let mut exact = raw.clone();
        exact.resize(65_536, b' ');
        assert_eq!(
            validate_config_binding(&exact, &expected_for(&exact)),
            Ok(())
        );
    }

    #[test]
    fn invalid_expected_request_is_rejected_before_config_parse() {
        let raw = bytes(&baseline());
        let mut expected = expected_for(&raw);
        expected.binding.adapter_sha256 = "short".into();
        assert_eq!(
            validate_config_binding(b"not-json", &expected),
            Err("invalid runtime request")
        );
    }

    #[test]
    fn errors_never_echo_sentinel_values() {
        let mut unknown = baseline();
        unknown["sentinel"] = json!("SENTINEL-CONFIG-DO-NOT-ECHO");
        let raw = bytes(&unknown);
        let error = validate_config_binding(&raw, &expected_for(&raw)).unwrap_err();
        assert_eq!(error, "invalid runtime config");
        assert!(!error.contains("SENTINEL"));
        assert!(!error.contains("scope-1"));
    }
}
