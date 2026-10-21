//! Independent read-only image readiness contract. No lease/mutation authority.
pub use crate::v2::{Engine, Observations, ObservedIdentity, Transport};
use crate::{digest, nonempty, Invalid, Result};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub enum Protocol {
    #[serde(rename = "gate-runtime/v3")]
    V3,
}
impl TryFrom<String> for Protocol {
    type Error = Invalid;
    fn try_from(s: String) -> Result<Self> {
        if s == "gate-runtime/v3" {
            Ok(Self::V3)
        } else {
            Err(Invalid("unsupported v3 protocol"))
        }
    }
}
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
    pub rootless: bool,
    pub api_version: String,
    pub server_version: String,
    pub daemon_id: String,
    pub scope: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub image: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub readiness: Target,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub binding: Binding,
    pub operation: Operation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Readiness {
    pub image_present: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "image_id"
    )]
    pub image_id: Option<String>,
}
fn image_id<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Readiness(Readiness),
    Unavailable,
}
impl<'de> Deserialize<'de> for Outcome {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Outcome;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("readiness object or unavailable string")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<Outcome, E> {
                if s == "unavailable" {
                    Ok(Outcome::Unavailable)
                } else {
                    Err(E::custom("invalid v3 outcome"))
                }
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut m: M,
            ) -> std::result::Result<Outcome, M::Error> {
                if m.next_key::<String>()?.as_deref() != Some("readiness") {
                    return Err(serde::de::Error::custom("invalid v3 outcome"));
                }
                let value = m.next_value()?;
                if m.next_key::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom("extra outcome fields"));
                }
                Ok(Outcome::Readiness(value))
            }
        }
        d.deserialize_any(Visitor)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub binding: Binding,
    pub operation: Operation,
    pub outcome: Outcome,
    #[serde(deserialize_with = "observations")]
    pub observations: Option<Observations>,
}
fn observations<'de, D: Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Observations>, D::Error> {
    Option::deserialize(d)
}

pub fn validate_image(image: &str) -> Result<()> {
    if image.is_empty()
        || image.len() > 512
        || !image.is_ascii()
        || image.starts_with(['-', '+'])
        || image
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control() || matches!(b, b'?' | b'#'))
        || image.split('/').any(|p| matches!(p, "" | "." | ".."))
    {
        return Err(Invalid("invalid v3 image reference"));
    }
    if let Some((repo, hash)) = image.split_once('@') {
        nonempty(repo)?;
        digest(
            hash.strip_prefix("sha256:")
                .ok_or(Invalid("unsupported image digest"))?,
        )?;
    }
    Ok(())
}
impl Binding {
    pub fn validate(&self) -> Result<()> {
        if self.adapter_version != "0.2.0"
            || self.invocation.len() != 32
            || !self
                .invocation
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Invalid("invalid v3 adapter or invocation"));
        }
        for d in [
            &self.adapter_sha256,
            &self.adapter_config_sha256,
            &self.operation_key,
            &self.generation,
        ] {
            digest(d)?;
        }
        for t in [&self.server_version, &self.daemon_id, &self.scope] {
            nonempty(t)?;
        }
        nonempty(&self.socket_path)?;
        if !self.socket_path.starts_with('/')
            || self.socket_path[1..]
                .split('/')
                .any(|p| matches!(p, "" | "." | ".."))
        {
            return Err(Invalid("invalid v3 socket path"));
        }
        let parts: Vec<_> = self.api_version.split('.').collect();
        if parts.len() != 2
            || parts
                .iter()
                .any(|p| !p.parse::<u16>().is_ok_and(|n| n.to_string() == *p))
        {
            return Err(Invalid("invalid v3 API version"));
        }
        Ok(())
    }
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        validate_image(&self.operation.readiness.image)
    }
}
impl Response {
    pub fn validate(&self, expected: &Request) -> Result<()> {
        expected.validate()?;
        if self.binding != expected.binding || self.operation != expected.operation {
            return Err(Invalid("v3 response binding mismatch"));
        }
        match (&self.outcome, &self.observations) {
            (Outcome::Unavailable, None) => Ok(()),
            (Outcome::Readiness(r), Some(pair)) => {
                for o in [&pair.observed_before, &pair.observed_after] {
                    if o.api_version != self.binding.api_version
                        || o.server_version != self.binding.server_version
                        || o.daemon_id != self.binding.daemon_id
                    {
                        return Err(Invalid("v3 observation mismatch"));
                    }
                }
                match (r.image_present, &r.image_id) {
                    (true, Some(id)) => digest(
                        id.strip_prefix("sha256:")
                            .ok_or(Invalid("invalid image ID"))?,
                    ),
                    (false, None) => Ok(()),
                    _ => Err(Invalid("inconsistent image readiness")),
                }
            }
            _ => Err(Invalid("invalid v3 observations")),
        }
    }
}
pub fn parse_request(bytes: &[u8]) -> Result<Request> {
    let r: Request = crate::parse(bytes)?;
    r.validate()?;
    Ok(r)
}
pub fn parse_response(bytes: &[u8], expected: &Request) -> Result<Response> {
    let r: Response = crate::parse(bytes)?;
    r.validate(expected)?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn request() -> Request {
        serde_json::from_value(json!({"binding":{"protocol":"gate-runtime/v3","adapter_sha256":"a".repeat(64),"adapter_version":"0.2.0","invocation":"1".repeat(32),"operation_key":"b".repeat(64),"generation":"c".repeat(64),"adapter_config_sha256":"d".repeat(64),"engine":"docker","transport":"unix-http","socket_path":"/run/test.sock","socket_uid":1000,"principal_uid":1000,"rootless":false,"api_version":"1.47","server_version":"27","daemon_id":"fixture","scope":"test"},"operation":{"readiness":{"image":"registry:5000/repo:tag"}}})).unwrap()
    }
    fn response() -> Value {
        let r = request();
        let o = json!({"api_version":"1.47","server_version":"27","daemon_id":"fixture"});
        json!({"binding":r.binding,"operation":r.operation,"outcome":{"readiness":{"image_present":true,"image_id":format!("sha256:{}","e".repeat(64))}},"observations":{"observed_before":o,"observed_after":o}})
    }
    fn bytes(v: &Value) -> Vec<u8> {
        serde_json::to_vec(v).unwrap()
    }
    #[test]
    fn readiness_requires_identity_on_present_and_missing_but_not_unavailable() {
        let expected = request();
        let mut value = response();
        assert!(parse_response(&bytes(&value), &expected).is_ok());
        value["outcome"] = json!({"readiness":{"image_present":false}});
        assert!(parse_response(&bytes(&value), &expected).is_ok());
        value["observations"] = Value::Null;
        assert!(parse_response(&bytes(&value), &expected).is_err());
        value["outcome"] = json!("unavailable");
        assert!(parse_response(&bytes(&value), &expected).is_ok());
        value.as_object_mut().unwrap().remove("observations");
        assert!(parse_response(&bytes(&value), &expected).is_err());
    }
    #[test]
    fn inconsistent_image_fields_and_capabilities_are_rejected() {
        for outcome in [
            json!({"readiness":{"image_present":true}}),
            json!({"readiness":{"image_present":true,"image_id":null}}),
            json!({"readiness":{"image_present":false,"image_id":null}}),
            json!({"readiness":{"image_present":false,"image_id":"sha256:abc"}}),
            json!({"readiness":{"image_present":true,"image_id":"bad"}}),
            json!({"capabilities":{"supported":["readiness"]}}),
            json!({"unavailable":null}),
            json!("absent"),
        ] {
            let mut v = response();
            v["outcome"] = outcome;
            assert!(parse_response(&bytes(&v), &request()).is_err());
        }
    }
    #[test]
    fn every_identity_and_exact_image_echo_are_required() {
        for field in [
            "protocol",
            "adapter_sha256",
            "adapter_version",
            "invocation",
            "operation_key",
            "generation",
            "adapter_config_sha256",
            "engine",
            "transport",
            "socket_path",
            "api_version",
            "server_version",
            "daemon_id",
            "scope",
        ] {
            let mut v = response();
            v["binding"][field] = json!("changed");
            assert!(parse_response(&bytes(&v), &request()).is_err(), "{field}");
        }
        for field in ["socket_uid", "principal_uid"] {
            let mut v = response();
            v["binding"][field] = json!(1001);
            assert!(parse_response(&bytes(&v), &request()).is_err());
        }
        let mut v = response();
        v["binding"]["rootless"] = json!(true);
        assert!(parse_response(&bytes(&v), &request()).is_err());
        v = response();
        v["operation"]["readiness"]["image"] = json!("other");
        assert!(parse_response(&bytes(&v), &request()).is_err());
        for side in ["observed_before", "observed_after"] {
            for field in ["api_version", "server_version", "daemon_id"] {
                let mut v = response();
                v["observations"][side][field] = json!("other");
                assert!(parse_response(&bytes(&v), &request()).is_err());
            }
        }
    }
    #[test]
    fn strict_duplicates_unknown_trailing_and_size_boundaries() {
        let r = serde_json::to_string(&request()).unwrap();
        for s in [
            r.replace("\"image\":", "\"image\":\"first\",\"image\":"),
            r.replace(
                "\"invocation\":",
                "\"invocation\":\"first\",\"invocation\":",
            ),
            format!("{r}{{}}"),
            r.replace("\"readiness\":{", "\"readiness\":{\"extra\":true,"),
        ] {
            assert!(parse_request(s.as_bytes()).is_err());
        }
        let response = String::from_utf8(bytes(&response())).unwrap();
        for s in [
            response.replace(
                "\"image_present\":true",
                "\"image_present\":true,\"image_present\":false",
            ),
            response.replace("\"readiness\":{", "\"readiness\":{\"extra\":true,"),
        ] {
            assert!(parse_response(s.as_bytes(), &request()).is_err());
        }
        let mut exact = r.into_bytes();
        exact.resize(crate::MAX_MESSAGE_BYTES, b' ');
        assert!(parse_request(&exact).is_ok());
        exact.push(b' ');
        assert!(parse_request(&exact).is_err());
    }
    #[test]
    fn image_reference_policy_and_v2_separation() {
        for s in [
            "",
            "-x",
            "+x",
            "a b",
            "a\n",
            "a?b",
            "a#b",
            "/a",
            "a//b",
            "a/../b",
            "a/./b",
            "é",
            "a@sha256:bad",
        ] {
            assert!(validate_image(s).is_err(), "{s}");
        }
        assert!(validate_image(&"x".repeat(512)).is_ok());
        assert!(validate_image(&"x".repeat(513)).is_err());
        assert!(validate_image(&format!("registry:5000/repo@sha256:{}", "a".repeat(64))).is_ok());
        let raw = serde_json::to_vec(&request()).unwrap();
        assert!(crate::v2::parse_request(&raw).is_err());
        for op in [
            json!("capabilities"),
            json!({"inspect":{"object_id":"a".repeat(64)}}),
            json!({"remove":{"object_id":"a".repeat(64)}}),
        ] {
            let mut v = serde_json::to_value(request()).unwrap();
            v["operation"] = op;
            assert!(parse_request(&bytes(&v)).is_err());
        }
    }
}
