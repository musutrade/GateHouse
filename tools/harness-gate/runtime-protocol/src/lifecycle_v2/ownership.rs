//! Exact selected ownership facts from native inspect, not journal authority.
//! Expected values must eventually come from a protected durable create plan.
use super::{
    native_json,
    native_status::{decode_inspect, inspect_route, InspectOutcome, State},
};
use serde::Deserialize;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Debug, Clone)]
pub struct ExpectedContainer {
    pub id: String,
    pub name: String,
    /// Native RootfsImageID, full lowercase hex without Docker sha256: prefix.
    pub image_id: String,
    /// Exact full merged label map, including any inherited image labels.
    pub labels: BTreeMap<String, String>,
    /// Fixed single TCP loopback mapping, or no published ports.
    pub port: Option<FixedPort>,
}
#[derive(Debug, Clone)]
pub struct FixedPort {
    pub container: u16,
    pub host: u16,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Object {
    name: String,
    image: String,
    config: Config,
    network_settings: Network,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Config {
    labels: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Network {
    ports: BTreeMap<String, Option<Vec<Port>>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Port {
    #[serde(rename = "HostIp")]
    host_ip: String,
    #[serde(rename = "HostPort")]
    host_port: String,
}
impl ExpectedContainer {
    pub fn validate(&self) -> Result<()> {
        inspect_route(&self.id)?;
        inspect_route(&self.image_id)?;
        if self.name.is_empty()
            || self.name.len() > 128
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || !self.name.as_bytes()[0].is_ascii_alphanumeric()
        {
            return Err("explicit container name required");
        }
        if self.labels.is_empty()
            || self.labels.len() > 64
            || self.labels.iter().any(|(k, v)| {
                k.is_empty()
                    || k.len() > 256
                    || v.len() > 4096
                    || k.chars().any(char::is_control)
                    || v.chars().any(char::is_control)
            })
        {
            return Err("bounded explicit label map required");
        }
        if self
            .port
            .as_ref()
            .is_some_and(|p| p.container == 0 || p.host == 0)
        {
            return Err("fixed nonzero ports required");
        }
        Ok(())
    }
    /// Present only: never convert a missing container into an owned object.
    /// Uses the same response bytes for state and ownership checks. It does not
    /// validate privilege, mounts, namespaces, env or other create-plan fields.
    pub fn match_inspect(
        &self,
        method: &str,
        route: &str,
        status: u16,
        body: &[u8],
    ) -> Result<State> {
        self.validate()?;
        let state = match decode_inspect(&self.id, method, route, status, body)? {
            InspectOutcome::Present(state) => state,
            InspectOutcome::Missing => {
                return Err("ownership cannot be proved for missing container")
            }
        };
        let object: Object = serde_json::from_value(native_json::decode(body)?)
            .map_err(|_| "native ownership fields rejected")?;
        if object.name != self.name
            || object.image != self.image_id
            || object.config.labels != self.labels
        {
            return Err("native ownership facts mismatch");
        }
        self.match_ports(&object.network_settings.ports)?;
        Ok(state)
    }
    fn match_ports(&self, ports: &BTreeMap<String, Option<Vec<Port>>>) -> Result<()> {
        match &self.port {
            None if ports.is_empty() => Ok(()),
            Some(expected) if ports.len() == 1 => {
                let key = format!("{}/tcp", expected.container);
                let bindings = ports
                    .get(&key)
                    .and_then(Option::as_ref)
                    .ok_or("expected published TCP port missing")?;
                if bindings.len() != 1
                    || bindings[0].host_ip != "127.0.0.1"
                    || bindings[0].host_port != expected.host.to_string()
                {
                    return Err("published port differs from exact plan");
                }
                Ok(())
            }
            _ => Err("unexpected native port map"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn expected() -> ExpectedContainer {
        ExpectedContainer {
            id: "a".repeat(64),
            name: "gate-fixture".into(),
            image_id: "b".repeat(64),
            labels: BTreeMap::from([
                ("io.gatehouse.lease".into(), "fixture-owned".into()),
                ("image.label".into(), "inherited".into()),
            ]),
            port: Some(FixedPort {
                container: 8080,
                host: 18080,
            }),
        }
    }
    fn fixture() -> Value {
        let e = expected();
        json!({"Id":e.id,"Name":e.name,"Image":e.image_id,"State":{"Status":"running","Running":true,"Paused":false,"Restarting":false,"Dead":false,"ExitCode":0},"Config":{"Labels":e.labels},"NetworkSettings":{"Ports":{"8080/tcp":[{"HostIp":"127.0.0.1","HostPort":"18080"}]}}})
    }
    fn check(e: &ExpectedContainer, v: &Value) -> Result<State> {
        e.match_inspect(
            "GET",
            &inspect_route(&e.id).unwrap(),
            200,
            &serde_json::to_vec(v).unwrap(),
        )
    }
    #[test]
    fn exact_owned_object_and_no_port_profile() {
        assert_eq!(check(&expected(), &fixture()).unwrap().status, "running");
        let mut e = expected();
        e.port = None;
        let mut v = fixture();
        v["NetworkSettings"]["Ports"] = json!({});
        assert!(check(&e, &v).is_ok());
    }
    #[test]
    fn exact_name_image_and_full_label_map_required() {
        for field in ["Name", "Image"] {
            let mut v = fixture();
            v[field] = "other".into();
            assert!(check(&expected(), &v).is_err());
        }
        for mode in 0..3 {
            let mut v = fixture();
            let labels = v["Config"]["Labels"].as_object_mut().unwrap();
            match mode {
                0 => {
                    labels.remove("io.gatehouse.lease");
                }
                1 => {
                    labels.insert("io.gatehouse.lease".into(), json!("other"));
                }
                _ => {
                    labels.insert("unexpected".into(), json!("value"));
                }
            }
            assert!(check(&expected(), &v).is_err());
        }
        let mut v = fixture();
        v["Image"] = format!("sha256:{}", expected().image_id).into();
        assert!(check(&expected(), &v).is_err());
    }
    #[test]
    fn missing_null_and_wrong_type_fields_refused() {
        for field in ["Name", "Image", "Config", "NetworkSettings"] {
            for missing in [true, false] {
                let mut v = fixture();
                if missing {
                    v.as_object_mut().unwrap().remove(field);
                } else {
                    v[field] = Value::Null;
                }
                assert!(check(&expected(), &v).is_err());
            }
        }
        let mut v = fixture();
        v["Config"]["Labels"] = Value::Null;
        assert!(check(&expected(), &v).is_err());
        let mut v = fixture();
        v["NetworkSettings"]["Ports"] = Value::Null;
        assert!(check(&expected(), &v).is_err());
    }
    #[test]
    fn no_extra_wildcard_udp_noncanonical_or_multiple_bindings() {
        for ports in [
            json!({}),
            json!({"8080/udp":[{"HostIp":"127.0.0.1","HostPort":"18080"}]}),
            json!({"8080/tcp":null}),
            json!({"8080/tcp":[]}),
            json!({"8080/tcp":[{"HostIp":"0.0.0.0","HostPort":"18080"}]}),
            json!({"8080/tcp":[{"HostIp":"127.0.0.1","HostPort":"018080"}]}),
            json!({"8080/tcp":[{"HostIp":"127.0.0.1","HostPort":"18081"}]}),
            json!({"8080/tcp":[{"HostIp":"127.0.0.1","HostPort":"18080"},{"HostIp":"::1","HostPort":"18080"}]}),
            json!({"8080/tcp":[{"HostIp":"127.0.0.1","HostPort":"18080"}],"90/tcp":null}),
        ] {
            let mut v = fixture();
            v["NetworkSettings"]["Ports"] = ports;
            assert!(check(&expected(), &v).is_err());
        }
    }
    #[test]
    fn invalid_expected_values_and_names_refused() {
        for name in ["", "/gate-fixture", "-name", "name/other"] {
            let mut e = expected();
            e.name = name.into();
            assert!(e.validate().is_err());
        }
        let mut e = expected();
        e.labels.clear();
        assert!(e.validate().is_err());
        let mut e = expected();
        e.port.as_mut().unwrap().host = 0;
        assert!(e.validate().is_err());
        let mut e = expected();
        e.image_id = "tag:latest".into();
        assert!(e.validate().is_err());
    }
    #[test]
    fn duplicate_fields_wrong_route_and_absence_never_own() {
        let e = expected();
        let route = inspect_route(&e.id).unwrap();
        let raw = serde_json::to_string(&fixture())
            .unwrap()
            .replace("\"HostIp\":", "\"HostIp\":\"0.0.0.0\",\"HostIp\":");
        assert!(e.match_inspect("GET", &route, 200, raw.as_bytes()).is_err());
        let body = serde_json::to_vec(&fixture()).unwrap();
        assert!(e.match_inspect("POST", &route, 200, &body).is_err());
        let absent=serde_json::to_vec(&json!({"cause":"no such container","message":format!("no container with name or ID \"{}\" found: no such container",e.id),"response":404})).unwrap();
        assert!(e.match_inspect("GET", &route, 404, &absent).is_err());
    }
}
