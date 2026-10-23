//! Native exact-ID image facts; no tag resolution, pull or absence authority.
use super::{
    native_create::CreatePlan,
    native_json,
    native_status::inspect_route,
    observation::{read_only_api, API},
    ownership::{validate_exposed, FixedPort},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, &'static str>;
#[derive(Debug)]
pub struct ImageFacts {
    id: String,
    labels: BTreeMap<String, String>,
    exposed: BTreeSet<String>,
    response_sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Image {
    id: String,
    labels: Option<BTreeMap<String, String>>,
    config: Config,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Config {
    labels: Option<BTreeMap<String, String>>,
    #[serde(default)]
    exposed_ports: BTreeMap<String, serde_json::Value>,
}
pub fn image_route(id: &str) -> Result<String> {
    image_route_for(API, id)
}
pub fn image_route_for(api: &str, id: &str) -> Result<String> {
    if !read_only_api(api) {
        return Err("unsupported native Podman API");
    }
    inspect_route(id)?;
    Ok(format!("/v{api}/libpod/images/{id}/json"))
}
pub fn decode_image(
    id: &str,
    method: &str,
    route: &str,
    status: u16,
    body: &[u8],
) -> Result<ImageFacts> {
    decode_image_for(API, id, method, route, status, body)
}
pub fn decode_image_for(
    api: &str,
    id: &str,
    method: &str,
    route: &str,
    status: u16,
    body: &[u8],
) -> Result<ImageFacts> {
    if method != "GET" || route != image_route_for(api, id)? || status != 200 {
        return Err("native image inspect unavailable");
    }
    let value = native_json::decode(body)?;
    if value.get("ExposedPorts").is_some() {
        return Err("image exposed ports must come from Config");
    }
    let image: Image = serde_json::from_value(value).map_err(|_| "native image fields rejected")?;
    if image.id != id {
        return Err("native image ID mismatch");
    }
    let labels = image.labels.unwrap_or_default();
    if labels != image.config.labels.unwrap_or_default() {
        return Err("native image label views disagree");
    }
    if labels.len() > 64
        || labels.iter().any(|(k, v)| {
            k.is_empty()
                || k.len() > 256
                || v.len() > 4096
                || k.chars().any(char::is_control)
                || v.chars().any(char::is_control)
        })
    {
        return Err("bounded image labels required");
    }
    let ports = image.config.exposed_ports;
    if ports
        .values()
        .any(|v| !v.as_object().is_some_and(|m| m.is_empty()))
    {
        return Err("image ExposedPorts values must be empty objects");
    }
    let exposed = ports.into_keys().collect();
    validate_exposed(&exposed)?;
    Ok(ImageFacts {
        id: id.into(),
        labels,
        exposed,
        response_sha256: format!("{:x}", Sha256::digest(body)),
    })
}
impl ImageFacts {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn response_sha256(&self) -> &str {
        &self.response_sha256
    }
    /// Exact image labels plus explicit operator overrides, following CompleteSpec.
    /// This is not a persisted reservation or permission to send the resulting plan.
    pub fn create_plan(
        &self,
        name: String,
        operator_labels: BTreeMap<String, String>,
        port: Option<FixedPort>,
    ) -> Result<CreatePlan> {
        let mut labels = self.labels.clone();
        labels.extend(operator_labels);
        CreatePlan::new(name, self.id.clone(), labels, port)?
            .with_image_exposed_ports(self.exposed.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn id() -> String {
        "b".repeat(64)
    }
    fn fixture() -> Value {
        json!({"Id":id(),"Labels":{"image":"value","override":"old"},"Config":{"Labels":{"image":"value","override":"old"},"ExposedPorts":{"80/tcp":{}}},"Digest":"","RepoTags":["mutable:tag"]})
    }
    fn parse(v: &Value) -> Result<ImageFacts> {
        decode_image(
            &id(),
            "GET",
            &image_route(&id()).unwrap(),
            200,
            &serde_json::to_vec(v).unwrap(),
        )
    }
    #[test]
    fn image_metadata_derives_exact_plan_without_publishing_expose() {
        let facts = parse(&fixture()).unwrap();
        let plan = facts
            .create_plan(
                "fixture".into(),
                BTreeMap::from([
                    ("override".into(), "new".into()),
                    ("lease".into(), "owned".into()),
                ]),
                None,
            )
            .unwrap();
        let body: Value = serde_json::from_slice(&plan.body().unwrap()).unwrap();
        assert_eq!(body["image"], id());
        assert_eq!(
            body["labels"],
            json!({"image":"value","override":"new","lease":"owned"})
        );
        assert_eq!(body["portmappings"], json!([]));
        let e = plan
            .expected_from_response(
                201,
                &serde_json::to_vec(&json!({"Id":"a".repeat(64),"Warnings":[]})).unwrap(),
            )
            .unwrap();
        assert_eq!(e.image_exposed_ports, BTreeSet::from(["80/tcp".into()]));
        assert_eq!(facts.id(), id());
        assert_eq!(
            facts.response_sha256(),
            format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&fixture()).unwrap())
            )
        );
    }
    #[test]
    fn labels_null_missing_empty_normalize_but_mismatch_refused() {
        for v in [
            json!({"Id":id(),"Config":{}}),
            json!({"Id":id(),"Labels":null,"Config":{"Labels":{},"ExposedPorts":{}}}),
        ] {
            assert!(parse(&v).is_ok());
        }
        let mut v = fixture();
        v["Labels"]["image"] = "other".into();
        assert!(parse(&v).is_err());
        for key in ["Id", "Config"] {
            let mut v = fixture();
            v.as_object_mut().unwrap().remove(key);
            assert!(parse(&v).is_err());
        }
        let mut v = fixture();
        v["Config"] = Value::Null;
        assert!(parse(&v).is_err());
    }
    #[test]
    fn id_route_status_and_no_digest_substitution() {
        for bad in ["tag:latest".into(), "b".repeat(63), "B".repeat(64)] {
            assert!(image_route(&bad).is_err());
        }
        let bytes = serde_json::to_vec(&fixture()).unwrap();
        let route = image_route(&id()).unwrap();
        for r in [
            route.replace("/libpod", ""),
            format!("{route}?size=true"),
            route.replace(&id(), &"c".repeat(64)),
        ] {
            assert!(decode_image(&id(), "GET", &r, 200, &bytes).is_err());
        }
        for status in [201, 204, 404, 500] {
            assert!(decode_image(&id(), "GET", &route, status, &bytes).is_err());
        }
        assert!(decode_image(&id(), "POST", &route, 200, &bytes).is_err());
        let mut v = fixture();
        v["Id"] = "c".repeat(64).into();
        v["Digest"] = format!("sha256:{}", id()).into();
        assert!(parse(&v).is_err());
    }
    #[test]
    fn expose_shape_keys_and_duplicate_labels_refused() {
        for exposed in [
            json!({"080/tcp":{}}),
            json!({"80/tcp":null}),
            json!({"80/tcp":{"x":true}}),
            json!([]),
        ] {
            let mut v = fixture();
            v["Config"]["ExposedPorts"] = exposed;
            assert!(parse(&v).is_err());
        }
        let mut v = fixture();
        v["ExposedPorts"] = json!({});
        assert!(parse(&v).is_err());
        let raw = serde_json::to_string(&fixture()).unwrap().replace(
            "\"image\":\"value\"",
            "\"image\":\"value\",\"image\":\"value\"",
        );
        assert!(decode_image(
            &id(),
            "GET",
            &image_route(&id()).unwrap(),
            200,
            raw.as_bytes()
        )
        .is_err());
    }
}
