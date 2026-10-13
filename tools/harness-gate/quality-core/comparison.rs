//! Exact nonnegative fractions using decimal digits, never machine floats.
use super::{error, json, require, schema, string, Result};
use serde_json::Value;
use std::cmp::Ordering;

fn multiply(a: &str, b: &str) -> Vec<u8> {
    let mut out = vec![0u8; a.len() + b.len()];
    for (i, x) in a.bytes().rev().enumerate() {
        let mut carry = 0u16;
        for (j, y) in b.bytes().rev().enumerate() {
            let n = u16::from(out[i + j]) + u16::from(x - b'0') * u16::from(y - b'0') + carry;
            out[i + j] = (n % 10) as u8;
            carry = n / 10;
        }
        out[i + b.len()] = carry as u8;
    }
    while out.len() > 1 && out.last() == Some(&0) {
        out.pop();
    }
    out.reverse();
    out
}

fn fraction(v: &Value) -> (String, String) {
    match string(&v["type"]) {
        "ratio" => (v["covered"].to_string(), v["total"].to_string()),
        "rational" => (v["numerator"].to_string(), v["denominator"].to_string()),
        "boolean" => (
            if v["value"] == true { "1" } else { "0" }.into(),
            "1".into(),
        ),
        "decimal" => {
            let value = string(&v["value"]).trim_end_matches('\n');
            let places = value.split_once('.').map_or(0, |(_, tail)| tail.len());
            (value.replace('.', ""), format!("1{}", "0".repeat(places)))
        }
        _ => (v["value"].to_string(), "1".into()),
    }
}

pub(super) fn validate(v: &Value) -> Result<()> {
    let definition = match v["type"].as_str() {
        Some("ratio") => "Ratio",
        Some("count") => "Count",
        Some("boolean") => "Boolean",
        Some("decimal") => "Decimal",
        Some("duration") => "Duration",
        Some("size") => "Size",
        Some("rational") => "Rational",
        _ => return Err(error("unknown value type")),
    };
    json::domain(v)?;
    schema::shape(v, &schema::EVIDENCE, Some(definition))?;
    if definition == "Ratio" {
        require(
            json::integer_cmp(&v["covered"], &v["total"]).is_le(),
            "ratio exceeds total",
        )?;
    }
    Ok(())
}

pub fn compare(observed: &Value, operator: &str, limit: &Value) -> Result<bool> {
    validate(observed)?;
    validate(limit)?;
    require(
        observed["type"] == limit["type"],
        "comparison type mismatch",
    )?;
    if observed["type"] == "boolean" {
        require(matches!(operator, "eq" | "ne"), "boolean requires eq or ne")?;
    }
    let (a, b) = fraction(observed);
    let (c, d) = fraction(limit);
    let left = multiply(&a, &d);
    let right = multiply(&c, &b);
    let order = left.len().cmp(&right.len()).then_with(|| left.cmp(&right));
    Ok(match operator {
        "lt" => order.is_lt(),
        "le" => order.is_le(),
        "eq" => order == Ordering::Equal,
        "ne" => order != Ordering::Equal,
        "ge" => order.is_ge(),
        "gt" => order.is_gt(),
        _ => return Err(error("unknown comparison operator")),
    })
}
