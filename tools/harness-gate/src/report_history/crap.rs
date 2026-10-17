//! Exact bounded chart coordinates; display quantization never evaluates policy.
use harness_gate::quality::policy::compare;
use serde_json::{json, Value};

pub(super) const MAXIMUM: u64 = 1_000_000_000;
pub(super) fn oversized(value: &Value) -> bool {
    match value["type"].as_str() {
        Some("decimal") => value["value"].as_str().is_some_and(|s| s.len() > 4096),
        Some("rational") => ["numerator", "denominator"].iter().any(|k| {
            value[*k]
                .as_number()
                .is_some_and(|n| n.to_string().len() > 4096)
        }),
        _ => false,
    }
}
fn boundary(kind: &str, ceiling: u64, grid: u64) -> Value {
    if kind == "rational" {
        json!({"type":"rational","numerator":ceiling*grid,"denominator":200})
    } else {
        // Small constant thresholds only: /200 = *5/1000, always a finite decimal.
        let units = ceiling * grid * 5;
        let text = format!("{}.{:03}", units / 1000, units % 1000);
        let text = text.trim_end_matches('0').trim_end_matches('.');
        json!({"type":"decimal","value":text})
    }
}
pub(super) fn candidate(value: &Value) -> Result<Value, &'static str> {
    if oversized(value) {
        return Err("CRAP field exceeds the 4096-character chart limit");
    }
    let kind = value["type"].as_str().unwrap_or("");
    if !matches!(kind, "decimal" | "rational") {
        return Err("unsupported CRAP chart value type");
    }
    match compare(value, "le", &boundary(kind, MAXIMUM, 200)) {
        Ok(true) => Ok(value.clone()),
        Ok(false) => Err("CRAP value exceeds the chart ceiling of 1000000000"),
        Err(_) => Err("invalid exact CRAP value"),
    }
}
fn ceilings() -> Vec<u64> {
    let mut result = Vec::new();
    let mut decade = 1;
    while decade < MAXIMUM {
        for factor in [1, 2, 5] {
            result.push(decade * factor);
        }
        decade *= 10;
    }
    result.push(MAXIMUM);
    result
}
pub(super) fn ceiling(value: &Value) -> u64 {
    let values = ceilings();
    let kind = value["type"].as_str().unwrap_or("");
    let mut lower = 0;
    let mut upper = values.len() - 1;
    while lower < upper {
        let middle = (lower + upper) / 2;
        if compare(value, "le", &boundary(kind, values[middle], 200)).unwrap_or(false) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    values[lower]
}
pub(super) fn coordinate(value: &Value, ceiling: u64) -> Result<u16, &'static str> {
    let kind = value["type"].as_str().unwrap_or("");
    let mut lower = 0_u64;
    let mut upper = 200_u64;
    while lower < upper {
        let middle = (lower + upper).div_ceil(2);
        match compare(value, "ge", &boundary(kind, ceiling, middle)) {
            Ok(true) => lower = middle,
            Ok(false) => upper = middle - 1,
            Err(_) => return Err("invalid exact CRAP coordinate"),
        }
    }
    Ok(lower as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_boundaries_are_canonical() {
        for (ceiling, grid, text) in [
            (1, 0, "0"),
            (1, 200, "1"),
            (1, 1, "0.005"),
            (1, 5, "0.025"),
            (1, 20, "0.1"),
        ] {
            assert_eq!(boundary("decimal", ceiling, grid)["value"], text);
        }
    }
    #[test]
    fn exact_coordinates_and_limits() {
        for (raw, expected) in [
            ("0", 0),
            ("0.004999999999999999999999", 0),
            ("0.005", 1),
            ("0.9999999999999999999999", 199),
            ("1", 200),
        ] {
            let value = candidate(&json!({"type":"decimal","value":raw})).unwrap();
            assert_eq!(coordinate(&value, 1), Ok(expected));
        }
        let third = candidate(&json!({"type":"rational","numerator":1,"denominator":3})).unwrap();
        assert_eq!(coordinate(&third, 1), Ok(66));
        assert_eq!(ceiling(&third), 1);
        assert_eq!(ceiling(&json!({"type":"decimal","value":"10.01"})), 20);
        for raw in [
            "9007199254740993",
            "18446744073709551616",
            "1000000000.0000000001",
        ] {
            assert!(candidate(&json!({"type":"decimal","value":raw})).is_err());
        }
        for value in [
            json!({"type":"rational","numerator":1,"denominator":0}),
            json!({"type":"decimal","value":"1e2"}),
            json!({"type":"count","value":3}),
            json!({"type":"decimal","value":"9".repeat(4097)}),
        ] {
            assert!(candidate(&value).is_err());
        }
        assert_eq!(
            coordinate(
                &candidate(&json!({"type":"decimal","value":"1000000000"})).unwrap(),
                MAXIMUM
            ),
            Ok(200)
        );
    }
    #[test]
    fn oversized_rational_fields_are_not_parsed() {
        for field in ["numerator", "denominator"] {
            let mut value = json!({"type":"rational","numerator":1,"denominator":1});
            value[field] = serde_json::from_str(&"9".repeat(4097)).unwrap();
            assert!(oversized(&value));
            assert!(candidate(&value).unwrap_err().contains("4096"));
        }
    }
    #[test]
    fn thirty_maximum_field_samples_are_bounded() {
        let started = std::time::Instant::now();
        let huge: Value = serde_json::from_str(&"9".repeat(4096)).unwrap();
        for i in 0..30 {
            if i % 3 == 2 {
                assert!(candidate(&json!({"type":"decimal","value":"9".repeat(4096)})).is_err());
                continue;
            }
            let value = if i % 3 == 0 {
                json!({"type":"decimal","value":format!("0.{}1","0".repeat(4093))})
            } else {
                json!({"type":"rational","numerator":huge,"denominator":huge})
            };
            let accepted = candidate(&value).unwrap();
            let top = ceiling(&accepted);
            assert!(coordinate(&accepted, top).unwrap() <= 200);
        }
        eprintln!(
            "30 maximum-field CRAP samples exact ceiling+coordinates: {:?}",
            started.elapsed()
        );
    }
}
