//! Integer-only canonical JSON. Reject duplicate keys before constructing Value.
use crate::{Invalid, Result, MAX_MESSAGE_BYTES};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;

struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded integer JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut out = Vec::new();
                while let Some(Strict(v)) = a.next_element()? {
                    out.push(v);
                }
                Ok(Strict(Value::Array(out)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Strict, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if out.contains_key(&k) {
                        return Err(de::Error::custom("duplicate field"));
                    }
                    let Strict(v) = a.next_value()?;
                    out.insert(k, v);
                }
                Ok(Strict(Value::Object(out)))
            }
        }
        d.deserialize_any(V)
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Value> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
        return Err(Invalid("invalid lifecycle message size"));
    }
    serde_json::from_slice::<Strict>(bytes)
        .map(|s| s.0)
        .map_err(|_| Invalid("invalid lifecycle JSON"))
}

pub(super) fn encode(value: &Value) -> Result<Vec<u8>> {
    fn append(value: &Value, out: &mut Vec<u8>) -> Result<()> {
        match value {
            Value::Object(map) => {
                out.push(b'{');
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    serde_json::to_writer(&mut *out, k)
                        .map_err(|_| Invalid("canonical encoding failed"))?;
                    out.push(b':');
                    append(&map[*k], out)?;
                }
                out.push(b'}');
            }
            Value::Array(items) => {
                out.push(b'[');
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    append(v, out)?;
                }
                out.push(b']');
            }
            Value::Number(n) if !(n.is_i64() || n.is_u64()) => {
                return Err(Invalid("noninteger JSON"))
            }
            _ => serde_json::to_writer(&mut *out, value)
                .map_err(|_| Invalid("canonical encoding failed"))?,
        }
        Ok(())
    }
    let mut out = Vec::new();
    append(value, &mut out)?;
    if out.len() > MAX_MESSAGE_BYTES {
        return Err(Invalid("invalid lifecycle message size"));
    }
    Ok(out)
}

pub(super) fn parse_canonical(bytes: &[u8]) -> Result<Value> {
    let value = parse(bytes)?;
    if encode(&value)? != bytes {
        return Err(Invalid("noncanonical lifecycle message"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_unknown_numeric_forms_and_trailing_data_rejected() {
        for b in [
            r#"{"a":1,"a":2}"#,
            r#"{"a":{"x":1,"x":2}}"#,
            r#"{"a":-0}"#,
            r#"{"a":1.0}"#,
            r#"{"a":1e0}"#,
            r#"{}{}"#,
        ] {
            assert!(parse_canonical(b.as_bytes()).is_err(), "{b}");
        }
    }
    #[test]
    fn canonical_order_unicode_escapes_and_whitespace() {
        let value = serde_json::json!({"z":"é\n\u{0001}/", "a":true});
        let bytes = encode(&value).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            "{\"a\":true,\"z\":\"é\\n\\u0001/\"}"
        );
        assert!(parse_canonical(&bytes).is_ok());
        assert!(parse_canonical(b"{ \"a\":true}").is_err());
        assert!(parse_canonical(br#"{"z":0,"a":true}"#).is_err());
    }
}
