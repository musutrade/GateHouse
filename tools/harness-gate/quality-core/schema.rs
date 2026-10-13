//! The frozen Python schema walker's subset, with semantic checks in the domains.
use super::{error, json, require, Result};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

pub(super) static PROJECT: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/project-model.schema.json")).unwrap()
});
pub(super) static EVIDENCE: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/harness-evidence.schema.json")).unwrap()
});
pub(super) static REQUIREMENTS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/capability-requirements.schema.json")).unwrap()
});
static PATTERNS: LazyLock<Mutex<HashMap<String, regex::Regex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) static POLICY: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("schema/policy.schema.json")).unwrap());
pub(super) static EXCEPTIONS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/policy-exceptions.schema.json")).unwrap()
});
pub(super) static MAPPINGS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("schema/subject-mappings.schema.json")).unwrap()
});

// Stable Python-style diagnostic values used by the frozen schema walker.
fn repr(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::Bool(v) => if *v { "True" } else { "False" }.into(),
        Value::String(v) => {
            let quote = if v.contains('\'') && !v.contains('"') {
                '"'
            } else {
                '\''
            };
            let mut out = String::from(quote);
            for c in v.chars() {
                match c {
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if c == quote => {
                        out.push('\\');
                        out.push(c);
                    }
                    c if c < ' ' || c == '\u{7f}' => out.push_str(&format!("\\x{:02x}", c as u32)),
                    c => out.push(c),
                }
            }
            out.push(quote);
            out
        }
        Value::Array(v) => format!("[{}]", v.iter().map(repr).collect::<Vec<_>>().join(", ")),
        _ => value.to_string(),
    }
}

pub(super) fn shape(value: &Value, schema: &Value, definition: Option<&str>) -> Result<()> {
    walk(
        value,
        definition.map_or(schema, |d| &schema["definitions"][d]),
        schema,
        "$",
    )
}

fn walk(v: &Value, s: &Value, root: &Value, path: &str) -> Result<()> {
    if let Some(reference) = s["$ref"].as_str() {
        return walk(
            v,
            root.pointer(reference.trim_start_matches('#'))
                .expect("embedded schema reference"),
            root,
            path,
        );
    }
    let fail = |message: String| error(format!("{path}: {message}"));
    if let Some(kind) = s.get("type") {
        let matches = |kind: &str| match kind {
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "integer" => json::integer(v),
            "boolean" => v.is_boolean(),
            "number" => v.is_number() && !json::integer(v),
            _ => false,
        };
        let valid = kind.as_str().map_or_else(
            || {
                kind.as_array()
                    .is_some_and(|k| k.iter().any(|x| matches(x.as_str().unwrap())))
            },
            matches,
        );
        if !valid {
            let expected = kind.as_array().map_or_else(
                || vec![kind.as_str().unwrap()],
                |k| k.iter().map(|v| v.as_str().unwrap()).collect(),
            );
            let expected = expected
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", ");
            let got = match v {
                Value::Null => "NoneType",
                Value::Bool(_) => "boolean",
                Value::Number(_) if json::integer(v) => "integer",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            return Err(fail(format!("expected type [{expected}], got {got}")));
        }
    }
    if let Some(c) = s.get("const") {
        require(
            v == c,
            format!("{path}: expected const {}, got {}", repr(c), repr(v)),
        )?;
    }
    if let Some(values) = s["enum"].as_array() {
        require(
            values.contains(v),
            format!(
                "{path}: expected one of {}, got {}",
                repr(&s["enum"]),
                repr(v)
            ),
        )?;
    }
    if let Some(text) = v.as_str() {
        if let Some(pattern) = s["pattern"].as_str() {
            let mut patterns = PATTERNS.lock().unwrap();
            let regex = patterns
                .entry(pattern.to_owned())
                .or_insert_with(|| regex::Regex::new(pattern).expect("embedded schema pattern"));
            // Python's `$` also matches before one final newline. The domain
            // traversal subsequently rejects that control character.
            require(
                regex.is_match(text)
                    || (pattern.ends_with('$')
                        && text.strip_suffix('\n').is_some_and(|s| regex.is_match(s))),
                format!(
                    "{path}: value {} does not match pattern {}",
                    repr(v),
                    repr(&Value::String(pattern.into()))
                ),
            )?;
        }
        for (key, ok) in [("minLength", true), ("maxLength", false)] {
            if let Some(limit) = s[key].as_u64() {
                require(
                    if ok {
                        text.chars().count() as u64 >= limit
                    } else {
                        text.chars().count() as u64 <= limit
                    },
                    format!(
                        "{path}: string is {} than {limit}",
                        if ok { "shorter" } else { "longer" }
                    ),
                )?;
            }
        }
    }
    if let Some(minimum) = s.get("minimum") {
        require(
            json::integer(v)
                && !v.to_string().starts_with('-')
                && json::integer_cmp(v, minimum).is_ge(),
            format!("{path}: integer {v} is below minimum {minimum}"),
        )?;
    }
    if let Some(object) = v.as_object() {
        if let Some(required) = s["required"].as_array() {
            let missing: Vec<_> = required
                .iter()
                .filter_map(|key| {
                    let key = key.as_str().unwrap();
                    (!object.contains_key(key)).then_some(key)
                })
                .collect();
            require(
                missing.is_empty(),
                format!("{path}: missing required field(s): {}", missing.join(", ")),
            )?;
        }
        if let Some(properties) = s["properties"].as_object() {
            for (key, child) in properties {
                if let Some(value) = object.get(key) {
                    walk(value, child, root, &format!("{path}.{key}"))?;
                }
            }
        }
        if s["additionalProperties"] == false {
            let unknown: Vec<_> = object
                .keys()
                .filter(|key| s["properties"].get(*key).is_none())
                .map(String::as_str)
                .collect();
            require(
                unknown.is_empty(),
                format!("{path}: unknown field(s): {}", unknown.join(", ")),
            )?;
        }
    }
    if let Some(values) = v.as_array() {
        if let Some(items) = s.get("items") {
            for (i, value) in values.iter().enumerate() {
                walk(value, items, root, &format!("{path}[{i}]"))?;
            }
        }
        for (key, lower) in [("minItems", true), ("maxItems", false)] {
            if let Some(limit) = s[key].as_u64() {
                require(
                    if lower {
                        values.len() as u64 >= limit
                    } else {
                        values.len() as u64 <= limit
                    },
                    format!("{path}: array violates {key}"),
                )?;
            }
        }
    }
    // Value.oneOf is intentionally resolved by the explicit metric discriminator,
    // as in the frozen reference. Metadata gets its own project traversal.
    Ok(())
}

/// Explicit compensation for `uniqueItems`, which the frozen walker subset
/// does not interpret (#313). Callers name the declaring field path.
pub(super) fn require_unique(values: &Value, path: &str) -> Result<()> {
    let values = values.as_array().map(Vec::as_slice).unwrap_or_default();
    let unique = values
        .iter()
        .enumerate()
        .all(|(i, value)| !values[..i].contains(value));
    require(unique, format!("{path}: array violates uniqueItems"))
}

#[cfg(test)]
mod keyword_tests {
    use super::*;

    /// Keywords `walk` enforces. `oneOf` is compensated by explicit
    /// discriminator checks in the policy and evidence domains.
    const ENFORCED: &[&str] = &[
        "$ref",
        "type",
        "const",
        "enum",
        "pattern",
        "minLength",
        "maxLength",
        "minimum",
        "required",
        "properties",
        "additionalProperties",
        "items",
        "minItems",
        "maxItems",
    ];
    /// `oneOf` uses explicit discriminator checks; `uniqueItems` uses
    /// `require_unique` at each declaring field (policy remediation classes).
    const COMPENSATED: &[&str] = &["oneOf", "uniqueItems"];
    const ANNOTATIONS: &[&str] = &["$schema", "$id", "title", "description", "definitions"];

    fn collect(schema: &Value, found: &mut Vec<String>) {
        match schema {
            Value::Object(object) => {
                for (key, child) in object {
                    found.push(key.clone());
                    match key.as_str() {
                        // Map keys under these are names, not keywords.
                        "properties" | "definitions" => {
                            for value in child.as_object().into_iter().flat_map(|o| o.values()) {
                                collect(value, found);
                            }
                        }
                        "const" | "enum" => {}
                        _ => collect(child, found),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect(item, found)),
            _ => {}
        }
    }

    #[test]
    fn every_embedded_schema_keyword_is_enforced_or_compensated() {
        for schema in [
            &*PROJECT,
            &*EVIDENCE,
            &*REQUIREMENTS,
            &*POLICY,
            &*EXCEPTIONS,
            &*MAPPINGS,
        ] {
            let mut found = Vec::new();
            collect(schema, &mut found);
            for keyword in found {
                assert!(
                    ENFORCED.contains(&keyword.as_str())
                        || COMPENSATED.contains(&keyword.as_str())
                        || ANNOTATIONS.contains(&keyword.as_str()),
                    "schema keyword {keyword:?} is neither enforced nor compensated"
                );
            }
        }
    }

    #[test]
    fn require_unique_rejects_duplicates_with_field_path() {
        require_unique(&serde_json::json!(["x", "y"]), "$.classes").unwrap();
        let error = require_unique(&serde_json::json!(["x", "x"]), "$.classes")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("$.classes: array violates uniqueItems"),
            "{error}"
        );
    }

    #[test]
    fn every_unique_items_declaration_is_compensated() {
        // Keep this in sync with `require_unique` call sites.
        let declared = POLICY.to_string().matches("\"uniqueItems\"").count();
        assert_eq!(
            declared, 1,
            "policy remediation_classes is the only declaration"
        );
        for schema in [
            &*PROJECT,
            &*EVIDENCE,
            &*REQUIREMENTS,
            &*EXCEPTIONS,
            &*MAPPINGS,
        ] {
            assert!(!schema.to_string().contains("\"uniqueItems\""));
        }
    }
}
