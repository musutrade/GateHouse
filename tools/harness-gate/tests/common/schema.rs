//! Minimal strict JSON Schema validator for machine-result corpus tests.
//! It supports exactly the keywords used by `schema/machine-result.schema.json`
//! and fails on any other keyword so the schema can never be weakened silently.
use serde_json::Value;

const KEYWORDS: &[&str] = &[
    "$ref",
    "$schema",
    "$id",
    "type",
    "enum",
    "const",
    "anyOf",
    "items",
    "minItems",
    "uniqueItems",
    "minLength",
    "minimum",
    "pattern",
    "format",
    "properties",
    "required",
    "additionalProperties",
    "definitions",
    "description",
    "title",
    "default",
];

pub fn load_machine_result_schema() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schema/machine-result.schema.json");
    serde_json::from_slice(&std::fs::read(path).expect("read machine-result schema"))
        .expect("parse machine-result schema")
}

pub fn validate_machine_result(instance: &Value) -> Result<(), String> {
    validate_document(&load_machine_result_schema(), instance)
}

pub fn validate_document(schema: &Value, instance: &Value) -> Result<(), String> {
    check_schema(schema, "#")?;
    validate(schema, instance, schema, "$")
}

fn check_schema(schema: &Value, path: &str) -> Result<(), String> {
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{path}: schema node must be an object"))?;
    for key in object.keys() {
        if !KEYWORDS.contains(&key.as_str()) {
            return Err(format!("{path}: unsupported schema keyword {key}"));
        }
    }
    for (name, child) in definitions(schema) {
        check_schema(&child, &format!("{path}/definitions/{name}"))?;
    }
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            check_schema(child, &format!("{path}/properties/{name}"))?;
        }
    }
    if let Some(child) = object.get("items") {
        check_schema(child, &format!("{path}/items"))?;
    }
    if let Some(child) = object.get("additionalProperties") {
        if child.is_object() {
            check_schema(child, &format!("{path}/additionalProperties"))?;
        }
    }
    if let Some(branches) = object.get("anyOf").and_then(Value::as_array) {
        for (index, child) in branches.iter().enumerate() {
            check_schema(child, &format!("{path}/anyOf/{index}"))?;
        }
    }
    Ok(())
}

fn definitions(schema: &Value) -> Vec<(String, Value)> {
    schema
        .get("definitions")
        .and_then(Value::as_object)
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

fn resolve_ref<'a>(root: &'a Value, reference: &str) -> Result<&'a Value, String> {
    let pointer = reference
        .strip_prefix('#')
        .ok_or_else(|| format!("unsupported external reference {reference}"))?;
    root.pointer(pointer)
        .ok_or_else(|| format!("unresolved reference {reference}"))
}

fn validate(schema: &Value, instance: &Value, root: &Value, path: &str) -> Result<(), String> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let target = resolve_ref(root, reference)?;
        return validate(target, instance, root, path);
    }
    if let Some(expected) = schema.get("type") {
        if !type_matches(expected, instance) {
            return Err(format!("{path}: type {expected} does not match {instance}"));
        }
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        if !values.iter().any(|value| value == instance) {
            return Err(format!("{path}: {instance} not in enum {schema:?}"));
        }
    }
    if let Some(constant) = schema.get("const") {
        if constant != instance {
            return Err(format!("{path}: {instance} != const {constant}"));
        }
    }
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        if !branches
            .iter()
            .any(|branch| validate(branch, instance, root, path).is_ok())
        {
            return Err(format!("{path}: no anyOf branch matched {instance}"));
        }
    }
    if let Some(items) = schema.get("items") {
        if let Some(array) = instance.as_array() {
            for (index, item) in array.iter().enumerate() {
                validate(items, item, root, &format!("{path}/{index}"))?;
            }
        }
    }
    if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64) {
        if let Some(array) = instance.as_array() {
            let length = array.len() as u64;
            if length < minimum {
                return Err(format!("{path}: {length} items < minItems {minimum}"));
            }
        }
    }
    if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
        if let Some(array) = instance.as_array() {
            for (index, item) in array.iter().enumerate() {
                if array[..index].contains(item) {
                    return Err(format!("{path}/{index}: duplicate array item"));
                }
            }
        }
    }
    if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64) {
        if let Some(text) = instance.as_str() {
            let length = text.chars().count() as u64;
            if length < minimum {
                return Err(format!("{path}: {length} chars < minLength {minimum}"));
            }
        }
    }
    if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64) {
        if let Some(number) = instance.as_f64() {
            if number < minimum {
                return Err(format!("{path}: {number} < minimum {minimum}"));
            }
        }
    }
    if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
        if let Some(text) = instance.as_str() {
            if !pattern_matches(pattern, text)? {
                return Err(format!(
                    "{path}: {text:?} does not match pattern {pattern:?}"
                ));
            }
        }
    }
    if let Some(format) = schema.get("format").and_then(Value::as_str) {
        if format == "date-time" {
            if let Some(text) = instance.as_str() {
                chrono::DateTime::parse_from_rfc3339(text)
                    .map_err(|error| format!("{path}: invalid date-time: {error}"))?;
            }
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        if let Some(object) = instance.as_object() {
            for (name, child) in properties {
                if let Some(value) = object.get(name) {
                    validate(child, value, root, &format!("{path}/{name}"))?;
                }
            }
        }
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        if let Some(object) = instance.as_object() {
            for name in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(name) {
                    return Err(format!("{path}: missing required property {name}"));
                }
            }
        }
    }
    if let Some(additional) = schema.get("additionalProperties") {
        if let Some(object) = instance.as_object() {
            let known = schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| {
                    properties
                        .keys()
                        .cloned()
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            for (name, value) in object {
                if known.contains(name) {
                    continue;
                }
                match additional {
                    Value::Bool(false) => {
                        return Err(format!(
                            "{path}: unexpected property {name} (additionalProperties=false)"
                        ));
                    }
                    Value::Bool(true) => {}
                    child => validate(child, value, root, &format!("{path}/{name}"))?,
                }
            }
        }
    }
    Ok(())
}

fn pattern_matches(pattern: &str, text: &str) -> Result<bool, String> {
    if let Ok(regex) = regex::Regex::new(pattern) {
        return Ok(regex.is_match(text));
    }
    // The schema contains one PCRE-style negative look-ahead (the relative
    // path guard) that the Rust regex crate cannot compile. Validate it exactly
    // rather than skipping it, and reject any other unsupported construct.
    const RELATIVE_PATH: &str = r"^(?!/)(?!.*(?:^|/)\.\.(?:/|$)).+";
    if pattern == RELATIVE_PATH {
        return Ok(!text.is_empty()
            && !text.starts_with('/')
            && !text.split('/').any(|segment| segment == ".."));
    }
    Err(format!("unsupported pattern {pattern:?}"))
}

fn type_matches(expected: &Value, instance: &Value) -> bool {
    match expected {
        Value::String(name) => type_name_matches(name, instance),
        Value::Array(names) => names
            .iter()
            .filter_map(Value::as_str)
            .any(|name| type_name_matches(name, instance)),
        _ => false,
    }
}

fn type_name_matches(name: &str, instance: &Value) -> bool {
    match name {
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "number" => instance.is_number(),
        "integer" => {
            instance.is_i64()
                || instance.is_u64()
                || instance
                    .as_f64()
                    .is_some_and(|number| number.fract() == 0.0)
        }
        _ => false,
    }
}
