//! Restricted native SpecGenerator encoding. No dispatch or durable authority.
use super::{
    native_exec::decode_created,
    ownership::{validate_plan, ExpectedContainer, FixedPort},
};
use serde::Serialize;
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, &'static str>;
pub const CREATE_ROUTE: &str = "/v6.1.4/libpod/containers/create";

pub struct CreatePlan {
    name: String,
    image: String,
    labels: BTreeMap<String, String>,
    port: Option<FixedPort>,
}
#[derive(Serialize)]
struct Wire<'a> {
    name: &'a str,
    image: &'a str,
    labels: &'a BTreeMap<String, String>,
    env: BTreeMap<String, String>,
    env_host: bool,
    httpproxy: bool,
    netns: Namespace,
    portmappings: Vec<Mapping>,
    publish_image_ports: bool,
    privileged: bool,
    remove: bool,
    #[serde(rename = "removeImage")]
    remove_image: bool,
    restart_policy: &'a str,
}
#[derive(Serialize)]
struct Namespace {
    nsmode: &'static str,
}
#[derive(Serialize)]
struct Mapping {
    host_ip: &'static str,
    container_port: u16,
    host_port: u16,
    range: u16,
    protocol: &'static str,
}
impl CreatePlan {
    /// Image must already be a resolved local full ID. Labels must be the full
    /// merged image/operator map, derived elsewhere under protected provenance.
    pub fn new(
        name: String,
        image: String,
        labels: BTreeMap<String, String>,
        port: Option<FixedPort>,
    ) -> Result<Self> {
        validate_plan(&name, &image, &labels, port.as_ref())?;
        Ok(Self {
            name,
            image,
            labels,
            port,
        })
    }
    pub fn body(&self) -> Result<Vec<u8>> {
        let ports = self
            .port
            .as_ref()
            .map(|p| Mapping {
                host_ip: "127.0.0.1",
                container_port: p.container,
                host_port: p.host,
                range: 1,
                protocol: "tcp",
            })
            .into_iter()
            .collect();
        serde_json::to_vec(&Wire {
            name: &self.name,
            image: &self.image,
            labels: &self.labels,
            env: BTreeMap::new(),
            env_host: false,
            httpproxy: false,
            netns: Namespace { nsmode: "bridge" },
            portmappings: ports,
            publish_image_ports: false,
            privileged: false,
            remove: false,
            remove_image: false,
            restart_policy: "no",
        })
        .map_err(|_| "native create encoding rejected")
    }
    /// Only the exact successful response supplies the ID. Failure must be
    /// retained as unknown by the future journal; never discover or retry by name.
    pub fn expected_from_response(&self, status: u16, body: &[u8]) -> Result<ExpectedContainer> {
        let expected = ExpectedContainer {
            id: decode_created(status, body, true)?,
            name: self.name.clone(),
            image_id: self.image.clone(),
            labels: self.labels.clone(),
            port: self.port.clone(),
        };
        expected.validate()?;
        Ok(expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn plan(port: Option<FixedPort>) -> CreatePlan {
        CreatePlan::new(
            "fixture".into(),
            "b".repeat(64),
            BTreeMap::from([("lease".into(), "owned".into())]),
            port,
        )
        .unwrap()
    }
    #[test]
    fn exact_native_body_and_explicit_range() {
        let p = plan(Some(FixedPort {
            container: 8080,
            host: 18080,
        }));
        let expected = format!(
            r#"{{"name":"fixture","image":"{}","labels":{{"lease":"owned"}},"env":{{}},"env_host":false,"httpproxy":false,"netns":{{"nsmode":"bridge"}},"portmappings":[{{"host_ip":"127.0.0.1","container_port":8080,"host_port":18080,"range":1,"protocol":"tcp"}}],"publish_image_ports":false,"privileged":false,"remove":false,"removeImage":false,"restart_policy":"no"}}"#,
            "b".repeat(64)
        );
        assert_eq!(p.body().unwrap(), expected.as_bytes());
        let v: serde_json::Value = serde_json::from_slice(&plan(None).body().unwrap()).unwrap();
        assert_eq!(v["portmappings"], json!([]));
        for absent in ["restart_tries", "expose", "networks", "HostConfig"] {
            assert!(v.get(absent).is_none());
        }
    }
    #[test]
    fn response_id_is_only_source_of_expected_container() {
        let p = plan(None);
        let body = serde_json::to_vec(&json!({"Id":"a".repeat(64),"Warnings":[]})).unwrap();
        let expected = p.expected_from_response(201, &body).unwrap();
        assert_eq!(expected.id, "a".repeat(64));
        assert_eq!(expected.image_id, "b".repeat(64));
        assert_eq!(expected.labels["lease"], "owned");
        for status in [200, 204, 409, 500] {
            assert!(p.expected_from_response(status, &body).is_err());
        }
        assert!(p.expected_from_response(201, b"{}").is_err());
    }
    #[test]
    fn unresolved_image_and_dynamic_port_refused() {
        assert!(CreatePlan::new(
            "fixture".into(),
            "image:latest".into(),
            BTreeMap::from([("lease".into(), "owned".into())]),
            None
        )
        .is_err());
        assert!(CreatePlan::new(
            "fixture".into(),
            "b".repeat(64),
            BTreeMap::from([("lease".into(), "owned".into())]),
            Some(FixedPort {
                container: 8080,
                host: 0
            })
        )
        .is_err());
    }
}
