//! Data-only pack expansion. Measurement and policy semantics belong to pack files.
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    pack: String,
    bindings: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    quality: Value,
    files: BTreeMap<String, Value>,
}

pub(super) struct Composition {
    pub quality: crate::config::quality::QualityConfig,
    pub files: BTreeMap<String, String>,
}

pub(super) fn compose(recipe: &str, packs: &[(&str, &str)], project: &str) -> Result<Composition> {
    let bindings = BTreeMap::from([("project".into(), project.into())]);
    let mut quality = expand(
        serde_json::from_str(include_str!("../../presets/quality-base.json"))?,
        &bindings,
    )?;
    let mut files = BTreeMap::new();
    for selection in
        serde_json::from_value::<Vec<Selection>>(harness_gate::quality::parse(recipe)?)?
    {
        let source = packs
            .iter()
            .find(|(id, _)| *id == selection.pack)
            .with_context(|| format!("unknown pack {}", selection.pack))?
            .1;
        let pack: Pack = serde_json::from_value(expand(
            harness_gate::quality::parse(source)?,
            &selection.bindings,
        )?)?;
        merge(&mut quality, pack.quality)?;
        for (path, value) in pack.files {
            ensure!(
                path.starts_with(".harness-gate/packs/")
                    && std::path::Path::new(&path)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_))),
                "pack file must remain under .harness-gate/packs: {path}"
            );
            ensure!(
                files
                    .insert(path.clone(), serde_json::to_string_pretty(&value)? + "\n")
                    .is_none(),
                "duplicate pack file {path}"
            );
        }
    }
    Ok(Composition {
        quality: serde_json::from_value(quality)
            .context("decode composed quality configuration")?,
        files,
    })
}

fn expand(value: Value, bindings: &BTreeMap<String, String>) -> Result<Value> {
    Ok(match value {
        Value::String(value) => {
            let mut output = String::new();
            let mut rest = value.as_str();
            while let Some((prefix, tail)) = rest.split_once("${") {
                output.push_str(prefix);
                let (key, suffix) = tail.split_once('}').context("unterminated pack binding")?;
                output.push_str(
                    bindings
                        .get(key)
                        .with_context(|| format!("missing pack binding {key}"))?,
                );
                rest = suffix;
            }
            output.push_str(rest);
            Value::String(output)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|v| expand(v, bindings))
                .collect::<Result<_>>()?,
        ),
        Value::Object(values) => {
            let mut result = serde_json::Map::new();
            for (key, value) in values {
                let key = expand(Value::String(key), bindings)?
                    .as_str()
                    .unwrap()
                    .to_owned();
                ensure!(
                    result
                        .insert(key.clone(), expand(value, bindings)?)
                        .is_none(),
                    "duplicate expanded pack key {key}"
                );
            }
            Value::Object(result)
        }
        value => value,
    })
}

fn merge(destination: &mut Value, source: Value) -> Result<()> {
    match (destination, source) {
        (Value::Object(destination), Value::Object(source)) => {
            for (key, value) in source {
                if let Some(existing) = destination.get_mut(&key) {
                    merge(existing, value).with_context(|| format!("compose pack key {key}"))?;
                } else {
                    destination.insert(key, value);
                }
            }
        }
        (Value::Array(destination), Value::Array(source)) => {
            for value in source {
                if !destination.contains(&value) {
                    destination.push(value);
                }
            }
        }
        (destination, source) if *destination == source => {}
        _ => bail!("conflicting pack values"),
    }
    Ok(())
}
