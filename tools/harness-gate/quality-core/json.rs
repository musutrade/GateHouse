//! Lossless integer JSON and duplicate-key rejection at every nesting level.
use super::{error, require, Result};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{value::RawValue, Value};
use std::{collections::BTreeSet, fmt};

// Walk the original token stream: a Value has already discarded duplicate keys.
// No RawValue child buffers or recursive reparsing are needed on valid input.
struct Unique;
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(parser: D) -> std::result::Result<Self, D::Error> {
        parser.deserialize_any(Unique).map(|()| Unique)
    }
}

impl<'de> Visitor<'de> for Unique {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON with unique object keys")
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_str<E: serde::de::Error>(self, _: &str) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        while seq.next_element::<Unique>()?.is_some() {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if let Some(key) = keys.replace(key) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate JSON key: {key}"
                )));
            }
            map.next_value::<Unique>()?;
        }
        Ok(())
    }
}

fn unique(text: &str) -> Result<()> {
    let mut parser = serde_json::Deserializer::from_str(text);
    if Unique::deserialize(&mut parser)
        .and_then(|_| parser.end())
        .is_err()
    {
        // Preserve historical nested error locations and precedence. Only the
        // error path pays for the old recursive RawValue walk.
        unique_legacy(text)?;
    }
    Ok(())
}

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
            unique_legacy(value.get()).map_err(serde::de::Error::custom)?;
        }
        Ok(())
    }
}

fn unique_legacy(text: &str) -> Result<()> {
    let mut parser = serde_json::Deserializer::from_str(text);
    match text.trim_start().chars().next() {
        Some('{') => parser
            .deserialize_map(UniqueObject)
            .map_err(|e| error(e.to_string()))?,
        Some('[') => {
            let values =
                Vec::<Box<RawValue>>::deserialize(&mut parser).map_err(|e| error(e.to_string()))?;
            for value in values {
                unique_legacy(value.get())?;
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
mod parsing_tests {
    use super::*;

    fn legacy_parse(text: &str) -> Result<Value> {
        let value = serde_json::from_str(text).map_err(|e| error(e.to_string()))?;
        unique_legacy(text)?;
        Ok(value)
    }

    #[test]
    fn streaming_check_preserves_values_and_exact_errors() {
        let cases = [
            "null",
            "true",
            "false",
            "0",
            "-0",
            "-0.0",
            "1.25",
            "1e400",
            "1234567890123456789012345678901234567890",
            "-1234567890123456789012345678901234567890",
            r#""é😀\n\t\"\\""#,
            r#"{"empty":{},"array":[null,true,1]}"#,
            r#"{"a":1,"a":2}"#,
            r#"{"a":1,"\u0061":2}"#,
            r#"[{"a":{"x":1,"\u0078":2}}]"#,
            r#"{"a":1,"a":{"x":1,"x":2}}"#,
            r#"{"a":{"x":1,"x":2},"a":0}"#,
            r#"{"$serde_json::private::Number":"12345678901234567890"}"#,
            r#"{"$serde_json::private::Number":"1","$serde_json::private::Number":"2"}"#,
            r#"{"$serde_json::private::Number":null}"#,
            r#"{"a":0,"$serde_json::private::Number":"1"}"#,
            "",
            "NaN",
            "Infinity",
            "[1,]",
            "{} {}",
            r#""\ud800""#,
            r#"{"a":1,"a":2,"bad":]}"#,
        ];
        for text in cases {
            for (prefix, suffix) in [("", ""), ("[", "]"), ("{\"outer\":", "}")] {
                let input = format!("{prefix}{text}{suffix}");
                let expected = legacy_parse(&input);
                if expected.is_ok() {
                    let mut parser = serde_json::Deserializer::from_str(&input);
                    assert!(Unique::deserialize(&mut parser).is_ok(), "{input}");
                    assert!(parser.end().is_ok());
                }
                assert_eq!(parse(&input), expected, "{input}");
            }
        }
    }

    #[test]
    fn streaming_check_keeps_recursion_boundary_and_valid_fast_path() {
        for depth in [0, 1, 63, 125, 126, 127, 128, 129] {
            for leaf in ["0", r#"{"key":1}"#, r#"{"key":1,"key":2}"#] {
                let text = format!("{}{leaf}{}", "[".repeat(depth), "]".repeat(depth));
                let expected = legacy_parse(&text);
                if expected.is_ok() {
                    let mut parser = serde_json::Deserializer::from_str(&text);
                    assert!(Unique::deserialize(&mut parser).is_ok(), "{text}");
                    assert!(parser.end().is_ok());
                }
                assert_eq!(parse(&text), expected, "depth {depth}");
            }
        }
    }
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
