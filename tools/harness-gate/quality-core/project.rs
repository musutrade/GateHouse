//! Project ownership, canonical identity, and structural relationship semantics.
use super::{array, canonical, digest, index, require, schema, string, ReasonClass, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::UnicodeNormalization;

pub fn canonical_path(path: &str) -> Result<()> {
    require(path.nfc().eq(path.chars()), "path must use NFC Unicode")?;
    require(
        !path.chars().any(|c| c < ' ' || c == '\\' || c == ':'),
        "invalid source path",
    )?;
    require(
        path == "."
            || (!path.is_empty()
                && path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")),
        "path must be canonical and relative",
    )
}

fn contains(root: &str, path: &str) -> bool {
    root == "."
        || root == path
        || path
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('/'))
}

pub(super) fn metadata(value: &Value) -> Result<()> {
    match value {
        Value::String(s) => require(
            !s.chars().any(|c| c < ' '),
            "control character in model string",
        )?,
        Value::Array(values) => {
            for v in values {
                metadata(v)?;
            }
        }
        Value::Object(values) => {
            for (key, v) in values {
                if key == "metadata" {
                    require(
                        v.as_object().is_some_and(|m| {
                            m.values()
                                .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                        }),
                        "metadata values must be nonempty strings",
                    )?;
                    for child in v.as_object().unwrap().values() {
                        metadata(child)?;
                    }
                } else {
                    metadata(v)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn shape(value: &Value, definition: Option<&str>) -> Result<()> {
    schema::shape(value, &schema::PROJECT, definition)?;
    metadata(value)
}

pub fn subject_id(project_id: &str, subject: &Value) -> Result<String> {
    let result = (|| {
        shape(subject, Some("Subject"))?;
        canonical_path(string(&subject["path"]))?;
        let discriminator = string(&subject["discriminator"]);
        // Python str.strip also treats the four ASCII information separators as
        // whitespace; model string validation already rejects those controls.
        require(
            discriminator.trim() == discriminator && discriminator.nfc().eq(discriminator.chars()),
            "discriminator must be canonical",
        )?;
        if let Some(span) = subject.get("span") {
            let compare = super::json::integer_cmp(&span["start_line"], &span["end_line"])
                .then_with(|| super::json::integer_cmp(&span["start_column"], &span["end_column"]));
            require(compare.is_le(), "reversed source span")?;
        }
        let mut payload = json!({"identity_version":"subject-identity/v1", "project":project_id});
        for key in [
            "component",
            "target",
            "boundary",
            "kind",
            "path",
            "discriminator",
            "span",
            "source_sha256",
        ] {
            if let Some(value) = subject.get(key) {
                payload[key] = value.clone();
            }
        }
        // Metadata is deliberately excluded from identity.
        Ok(format!(
            "subject-identity/v1:{}",
            digest(&canonical(&payload)?)
        ))
    })();
    result.map_err(|mut e: super::Error| {
        e.class = ReasonClass::ModelError;
        e
    })
}

fn unique<'a>(records: &'a Value, label: &str) -> Result<BTreeMap<&'a str, &'a Value>> {
    index(records, "id", &format!("{label} identity"))
}

pub fn validate_project(project: &Value) -> Result<super::model::Project> {
    validate(project)
        .and_then(|()| super::json::decode(project))
        .map_err(|mut e| {
            e.class = ReasonClass::ModelError;
            e
        })
}

fn validate(project: &Value) -> Result<()> {
    shape(project, None)?;
    let components = unique(&project["components"], "component")?;
    let mut roots = BTreeSet::new();
    let mut targets = BTreeMap::new();
    let mut boundaries = BTreeMap::new();
    for component in array(&project["components"]) {
        let id = string(&component["id"]);
        let root = string(&component["path"]);
        canonical_path(root)?;
        require(roots.insert(root), "duplicate component path")?;
        targets.insert(id, unique(&component["targets"], "target")?);
        let local = unique(&component["source_boundaries"], "boundary")?;
        for boundary in array(&component["source_boundaries"]) {
            let path = string(&boundary["path"]);
            canonical_path(path)?;
            require(contains(root, path), "boundary escapes component")?;
        }
        for target in array(&component["targets"]) {
            let refs = array(&target["boundaries"]);
            let set: BTreeSet<_> = refs.iter().map(string).collect();
            require(refs.len() == set.len(), "duplicate target boundary")?;
            require(
                set.iter().all(|id| local.contains_key(id)),
                "unknown target boundary",
            )?;
        }
        boundaries.insert(id, local);
    }
    let subjects = unique(&project["subjects"], "subject")?;
    let mut locations = BTreeSet::new();
    let mut sources = BTreeMap::new();
    // Iterate in input order to preserve the reference's first failure.
    for subject in array(&project["subjects"]) {
        let component = string(&subject["component"]);
        let target = string(&subject["target"]);
        let boundary = string(&subject["boundary"]);
        require(
            components.contains_key(component),
            "unknown subject component",
        )?;
        require(
            targets[component].contains_key(target),
            "unknown subject target",
        )?;
        require(
            array(&targets[component][target]["boundaries"]).contains(&subject["boundary"]),
            "unknown subject boundary for target",
        )?;
        let path = string(&subject["path"]);
        canonical_path(path)?;
        require(
            contains(string(&boundaries[component][boundary]["path"]), path),
            "subject escapes source boundary",
        )?;
        require(
            subject["id"] == subject_id(string(&project["id"]), subject)?,
            "noncanonical subject identity",
        )?;
        let source = (component, string(&subject["source_sha256"]));
        require(
            sources.insert(path, source).is_none_or(|old| old == source),
            "conflicting source path or digest",
        )?;
        let locator: Vec<_> = [
            "component",
            "target",
            "boundary",
            "kind",
            "path",
            "discriminator",
            "span",
        ]
        .iter()
        .map(|k| subject[k].clone())
        .collect();
        require(
            locations.insert(canonical(&json!(locator))?),
            "duplicate subject location",
        )?;
    }
    unique(&project["relationships"], "relationship")?;
    let mut edges = BTreeSet::new();
    for relationship in array(&project["relationships"]) {
        let producer = string(&relationship["producer"]);
        let consumer = string(&relationship["consumer"]);
        require(
            components.contains_key(producer) && components.contains_key(consumer),
            "unknown relationship component",
        )?;
        require(producer != consumer, "relationship must cross components")?;
        let refs = array(&relationship["subjects"]);
        let set: BTreeSet<_> = refs.iter().map(string).collect();
        require(refs.len() == set.len(), "duplicate relationship subject")?;
        require(
            set.iter().all(|id| subjects.contains_key(id)),
            "unknown relationship subject identity",
        )?;
        require(
            set.iter()
                .all(|id| [producer, consumer].contains(&string(&subjects[id]["component"]))),
            "relationship subject belongs to an unrelated component",
        )?;
        require(
            edges.insert((string(&relationship["kind"]), producer, consumer, set)),
            "duplicate relationship edge",
        )?;
    }
    Ok(())
}
