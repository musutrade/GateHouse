use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Deterministic identity parsed from a Docker/Podman inspection document.
///
/// This type and its parser are service-core: they validate the immutable
/// object identity and ownership labels that lease cleanup compares before a
/// destructive action. Keeping the parsing here means fake-runtime tests can
/// exercise the real ownership checks without a local daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeInspection {
    pub(crate) object_id: String,
    pub(crate) name: String,
    pub(crate) labels: BTreeMap<String, String>,
}

pub(crate) fn parse_runtime_inspection(raw: &[u8]) -> Result<RuntimeInspection> {
    let value: serde_json::Value =
        serde_json::from_slice(raw).context("runtime inspection is not valid JSON")?;
    let object = value
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(&value);
    let object_id = object
        .get("Id")
        .or_else(|| object.get("ID"))
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("runtime inspection has no immutable object ID"))?
        .to_string();
    let name = object
        .get("Name")
        .or_else(|| {
            object
                .get("Names")
                .and_then(serde_json::Value::as_array)
                .and_then(|names| names.first())
        })
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string();
    let labels_value = object
        .get("Config")
        .and_then(|config| config.get("Labels"))
        .or_else(|| object.get("Labels"));
    let mut labels = BTreeMap::new();
    if let Some(map) = labels_value.and_then(serde_json::Value::as_object) {
        for (key, value) in map {
            let value = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("runtime label {key:?} is not a string"))?;
            labels.insert(key.clone(), value.to_string());
        }
    }
    Ok(RuntimeInspection {
        object_id,
        name,
        labels,
    })
}

pub(super) fn parse_mapped_port(raw: &[u8]) -> Option<String> {
    String::from_utf8_lossy(raw)
        .lines()
        .next()
        .and_then(|line| line.rsplit(':').next())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::parse_runtime_inspection;

    #[test]
    fn parses_docker_style_inspection() {
        let value = parse_runtime_inspection(
            br#"[{"Id":"sha256:abc","Name":"/fixture","Config":{"Labels":{"harness-gate.owner":"harness-gate"}}}]"#,
        )
        .expect("inspection");
        assert_eq!(value.object_id, "sha256:abc");
        assert_eq!(value.name, "fixture");
        assert_eq!(
            value.labels.get("harness-gate.owner"),
            Some(&"harness-gate".into())
        );
    }

    #[test]
    fn rejects_inspection_without_an_immutable_object_id() {
        let error = parse_runtime_inspection(br#"{"Name":"/fixture"}"#).expect_err("object id");
        assert!(format!("{error:#}").contains("immutable object ID"));
    }

    #[test]
    fn rejects_inspection_that_is_not_json() {
        let error = parse_runtime_inspection(b"not-json").expect_err("parse failure");
        assert!(format!("{error:#}").contains("not valid JSON"));
    }
}
