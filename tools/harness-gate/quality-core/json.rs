//! Lossless integer JSON and duplicate-key rejection at every nesting level.
use super::{error, require, Result};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{value::RawValue, Value};
use std::{collections::BTreeSet, fmt};

struct UniqueObject;
impl<'de> Visitor<'de> for UniqueObject {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an object with unique keys")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON key: {key}"
                )));
            }
            unique(value.get()).map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}

fn unique(text: &str) -> Result<()> {
    let mut parser = serde_json::Deserializer::from_str(text);
    match text.trim_start().chars().next() {
        Some('{') => parser
            .deserialize_map(UniqueObject)
            .map_err(|e| error(e.to_string()))?,
        Some('[') => {
            let values =
                Vec::<Box<RawValue>>::deserialize(&mut parser).map_err(|e| error(e.to_string()))?;
            for value in values {
                unique(value.get())?;
            }
        }
        _ => {
            Box::<RawValue>::deserialize(&mut parser).map_err(|e| error(e.to_string()))?;
        }
    }
    parser.end().map_err(|e| error(e.to_string()))
}

pub fn parse(text: &str) -> Result<Value> {
    // Apply serde_json's syntax and recursion limit before the duplicate walk.
    let value = serde_json::from_str(text).map_err(|e| error(e.to_string()))?;
    unique(text)?;
    Ok(value)
}

pub(super) fn integer(value: &Value) -> bool {
    value
        .as_number()
        .is_some_and(|n| !n.to_string().contains(['.', 'e', 'E']))
}

/// Compare arbitrary-precision JSON integers by sign, digit count and digits.
/// serde_json integers carry no leading zeros or exponent, so this is exact
/// for negative values too (#318).
pub(super) fn integer_cmp(a: &Value, b: &Value) -> std::cmp::Ordering {
    let (a, b) = (a.to_string(), b.to_string());
    let magnitude = |x: &str, y: &str| x.len().cmp(&y.len()).then_with(|| x.cmp(y));
    match (a.strip_prefix('-'), b.strip_prefix('-')) {
        (None, None) => magnitude(&a, &b),
        (Some(a), Some(b)) => magnitude(b, a),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
    }
}

pub(super) fn domain(value: &Value) -> Result<()> {
    match value {
        Value::String(s) => require(
            !s.chars().any(|c| c < ' '),
            "invalid character in evidence string",
        )?,
        Value::Array(values) => {
            for v in values {
                domain(v)?;
            }
        }
        Value::Object(values) => {
            for (k, v) in values {
                domain(&Value::String(k.clone()))?;
                domain(v)?;
            }
        }
        Value::Number(_) => require(integer(value), "untyped number or non-JSON value")?,
        _ => {}
    }
    Ok(())
}

pub fn canonical(value: &Value) -> Result<Vec<u8>> {
    domain(value)?;
    // serde_json's default map uses sorted keys; strings remain UTF-8 and integers
    // use arbitrary precision, matching the Python identity contract.
    serde_json::to_vec(value).map_err(|e| error(e.to_string()))
}

// Deserialize from JSON text, not Value's numeric deserializer: the latter may
// round-trip a large integer through f64 (e.g. 10^100 becomes 1e+100).
pub(super) fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T> {
    serde_json::from_str(&value.to_string()).map_err(|e| error(e.to_string()))
}

#[cfg(test)]
mod integer_tests {
    use super::integer_cmp;
    use serde_json::json;
    use std::cmp::Ordering::{Equal, Greater, Less};

    #[test]
    fn integer_cmp_orders_signed_and_arbitrary_precision_integers() {
        let big: serde_json::Value =
            serde_json::from_str("123456789012345678901234567890").unwrap();
        for (a, b, expected) in [
            (json!(-5), json!(3), Less),
            (json!(3), json!(-5), Greater),
            (json!(-10), json!(-9), Less),
            (json!(-9), json!(-10), Greater),
            (json!(-7), json!(-7), Equal),
            (json!(0), json!(-1), Greater),
            (json!(10), json!(9), Greater),
            (json!(42), json!(42), Equal),
            (big.clone(), json!(u64::MAX), Greater),
        ] {
            assert_eq!(integer_cmp(&a, &b), expected, "{a} vs {b}");
        }
    }
}
