use crate::config::ParserConfig;
use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ShardResult {
    pub shard_index: u32,
    pub shard_total: u32,
    pub test_ids: Vec<String>,
}

/// Validate and merge a complete shard set. The merge identity is the stable
/// test ID rather than completion order, so duplicate or missing shards fail
/// closed instead of silently changing the gate count.
pub(crate) fn merge_shards(results: &[ShardResult], expected_total: u32) -> Result<Vec<String>> {
    if expected_total == 0 {
        return Err(anyhow::anyhow!("shard total must be greater than zero"));
    }
    if results.len() != expected_total as usize {
        return Err(anyhow::anyhow!(
            "missing shard: expected {expected_total}, received {}",
            results.len()
        ));
    }
    let mut seen_shards = std::collections::BTreeSet::new();
    let mut seen_tests = std::collections::BTreeSet::new();
    for result in results {
        if result.shard_total != expected_total {
            return Err(anyhow::anyhow!(
                "shard {} declares total {}, expected {expected_total}",
                result.shard_index,
                result.shard_total
            ));
        }
        if result.shard_index >= expected_total || !seen_shards.insert(result.shard_index) {
            return Err(anyhow::anyhow!(
                "duplicate or out-of-range shard {}",
                result.shard_index
            ));
        }
        for test_id in &result.test_ids {
            if !seen_tests.insert(test_id.clone()) {
                return Err(anyhow::anyhow!(
                    "duplicate test identity {test_id:?} across shards"
                ));
            }
        }
    }
    if seen_shards.len() != expected_total as usize {
        return Err(anyhow::anyhow!("missing shard in declared set"));
    }
    Ok(seen_tests.into_iter().collect())
}

pub(super) fn parse_result_count(content: &str, parser: &ParserConfig) -> Result<(usize, usize)> {
    let ansi = Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]")?;
    let normalized = ansi.replace_all(content, "");
    match parser {
        ParserConfig::Regex {
            patterns,
            capture,
            minimum,
        } => {
            let regexes = patterns
                .iter()
                .map(|pattern| Regex::new(pattern))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut count = 0;
            for regex in regexes {
                count += regex
                    .captures_iter(&normalized)
                    .filter_map(|captures| captures.get(*capture)?.as_str().parse::<usize>().ok())
                    .sum::<usize>();
            }
            Ok((count, *minimum))
        }
        ParserConfig::Junit { minimum } => Ok((
            count_xml_elements(
                normalized.as_ref(),
                b"testcase",
                &[b"testsuite", b"testsuites"],
            )?,
            *minimum,
        )),
        ParserConfig::Trx { minimum } => Ok((
            count_xml_elements(normalized.as_ref(), b"UnitTestResult", &[b"TestRun"])?,
            *minimum,
        )),
        ParserConfig::Json {
            count_path,
            minimum,
        } => {
            let value: Value = serde_json::from_str(normalized.as_ref())
                .map_err(|error| anyhow::anyhow!("parse JSON test results: {error}"))?;
            let count = count_json_results(&value, count_path.as_deref())?;
            Ok((count, *minimum))
        }
    }
}

fn count_xml_elements(content: &str, wanted: &[u8], allowed_roots: &[&[u8]]) -> Result<usize> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);
    let mut count = 0;
    let mut stack = Vec::<Vec<u8>>::new();
    let mut seen_root = false;
    let mut root_closed = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = event.name();
                let local_name = name.local_name();
                if stack.is_empty() {
                    if root_closed {
                        return Err(anyhow::anyhow!(
                            "parse XML test results: multiple root elements"
                        ));
                    }
                    if !allowed_roots
                        .iter()
                        .any(|root| *root == local_name.as_ref())
                    {
                        return Err(anyhow::anyhow!(
                            "parse XML test results: invalid root element {:?}",
                            String::from_utf8_lossy(local_name.as_ref())
                        ));
                    }
                    seen_root = true;
                }
                if local_name.as_ref() == wanted {
                    count += 1;
                }
                stack.push(name.as_ref().to_vec());
            }
            Ok(Event::Empty(event)) => {
                let name = event.name();
                let local_name = name.local_name();
                if stack.is_empty() {
                    if root_closed {
                        return Err(anyhow::anyhow!(
                            "parse XML test results: multiple root elements"
                        ));
                    }
                    if !allowed_roots
                        .iter()
                        .any(|root| *root == local_name.as_ref())
                    {
                        return Err(anyhow::anyhow!(
                            "parse XML test results: invalid root element {:?}",
                            String::from_utf8_lossy(local_name.as_ref())
                        ));
                    }
                    seen_root = true;
                    root_closed = true;
                } else if local_name.as_ref() == wanted {
                    count += 1;
                }
            }
            Ok(Event::End(event)) => {
                let Some(open) = stack.pop() else {
                    return Err(anyhow::anyhow!(
                        "parse XML test results: unexpected closing tag"
                    ));
                };
                if open != event.name().as_ref() {
                    return Err(anyhow::anyhow!(
                        "parse XML test results: mismatched closing tag"
                    ));
                }
                if stack.is_empty() {
                    root_closed = true;
                }
            }
            Ok(Event::Text(event)) => {
                if stack.is_empty() && event.iter().any(|byte| !byte.is_ascii_whitespace()) {
                    return Err(anyhow::anyhow!(
                        "parse XML test results: non-whitespace content outside root"
                    ));
                }
            }
            Ok(Event::CData(_)) => {
                if stack.is_empty() {
                    return Err(anyhow::anyhow!(
                        "parse XML test results: CDATA outside root"
                    ));
                }
            }
            Ok(Event::Decl(_)) if seen_root => {
                return Err(anyhow::anyhow!(
                    "parse XML test results: XML declaration outside prolog"
                ));
            }
            Ok(Event::DocType(_)) if seen_root => {
                return Err(anyhow::anyhow!(
                    "parse XML test results: doctype outside prolog"
                ));
            }
            Ok(Event::GeneralRef(_)) if stack.is_empty() => {
                return Err(anyhow::anyhow!(
                    "parse XML test results: entity reference outside root"
                ));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow::anyhow!("parse XML test results: {error}")),
        }
    }
    if !seen_root {
        return Err(anyhow::anyhow!(
            "parse XML test results: missing root element"
        ));
    }
    if !stack.is_empty() {
        return Err(anyhow::anyhow!("parse XML test results: unclosed tag"));
    }
    Ok(count)
}

fn count_json_path(value: &Value, path: &str) -> Result<usize> {
    if path.split('.').any(|segment| segment.is_empty()) {
        return Err(anyhow::anyhow!(
            "JSON result path {path:?} must be a non-empty dot path"
        ));
    }
    let mut current = value;
    for segment in path.split('.') {
        current = current
            .get(segment)
            .ok_or_else(|| anyhow::anyhow!("JSON result path {path:?} is missing"))?;
    }
    match current {
        Value::Array(items) => Ok(items.len()),
        Value::Number(number) => {
            let count = number.as_u64().ok_or_else(|| {
                anyhow::anyhow!("JSON result count at {path:?} is not a non-negative integer")
            })?;
            usize::try_from(count)
                .map_err(|_| anyhow::anyhow!("JSON result count at {path:?} does not fit in usize"))
        }
        _ => Err(anyhow::anyhow!(
            "JSON result path {path:?} must be an array or integer"
        )),
    }
}

const SUPPORTED_FIELDS: [&str; 5] = [
    "testcases",
    "testCases",
    "test_results",
    "testResults",
    "results",
];

fn discover_json_results(
    value: &Value,
    path: &str,
    candidates: &mut Vec<(String, usize)>,
) -> Result<()> {
    let Value::Object(map) = value else {
        return Ok(());
    };

    for field in SUPPORTED_FIELDS {
        let Some(candidate) = map.get(field) else {
            continue;
        };
        let candidate_path = if path.is_empty() {
            field.to_owned()
        } else {
            format!("{path}.{field}")
        };
        let Value::Array(items) = candidate else {
            return Err(anyhow::anyhow!(
                "JSON result field {candidate_path:?} must be an array"
            ));
        };
        candidates.push((candidate_path, items.len()));
    }

    // Only object wrappers may be traversed. In particular, arrays are
    // result containers, not search roots for unrelated nested values.
    for (field, child) in map {
        if SUPPORTED_FIELDS.contains(&field.as_str()) {
            continue;
        }
        if child.is_object() {
            let child_path = if path.is_empty() {
                field.to_owned()
            } else {
                format!("{path}.{field}")
            };
            discover_json_results(child, &child_path, candidates)?;
        }
    }
    Ok(())
}

fn count_json_results(value: &Value, path: Option<&str>) -> Result<usize> {
    if let Some(path) = path {
        return count_json_path(value, path);
    }

    if let Value::Array(items) = value {
        return Ok(items.len());
    }

    let mut candidates = Vec::new();
    discover_json_results(value, "", &mut candidates)?;
    match candidates.as_slice() {
        [] => Err(anyhow::anyhow!(
            "JSON test results contain no supported result array"
        )),
        [(path, count)] => {
            let _ = path;
            Ok(*count)
        }
        _ => {
            let paths = candidates
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(anyhow::anyhow!(
                "JSON test results contain ambiguous result arrays at {paths}"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_junit_testcases() {
        let parser = ParserConfig::Junit { minimum: 2 };
        assert_eq!(
            parse_result_count("<testsuite><testcase/><testcase/></testsuite>", &parser).unwrap(),
            (2, 2)
        );
    }

    #[test]
    fn parses_nested_junit_suites_and_namespaces() {
        let parser = ParserConfig::Junit { minimum: 1 };
        assert_eq!(
            parse_result_count(
                r#"<j:testsuites xmlns:j="urn:junit"><j:testsuite><j:testcase/></j:testsuite></j:testsuites>"#,
                &parser,
            )
            .unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn parses_trx_results() {
        let parser = ParserConfig::Trx { minimum: 1 };
        assert_eq!(
            parse_result_count("<TestRun><UnitTestResult testId=\"a\"/></TestRun>", &parser)
                .unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn parses_json_result_path() {
        let parser = ParserConfig::Json {
            count_path: Some("summary.total".into()),
            minimum: 3,
        };
        assert_eq!(
            parse_result_count(r#"{"summary":{"total":3}}"#, &parser).unwrap(),
            (3, 3)
        );
    }

    #[test]
    fn parses_json_explicit_array_path() {
        let parser = ParserConfig::Json {
            count_path: Some("summary.results".into()),
            minimum: 1,
        };
        assert_eq!(
            parse_result_count(r#"{"summary":{"results":[{},{}]}}"#, &parser).unwrap(),
            (2, 1)
        );
    }

    #[test]
    fn parses_json_explicit_non_negative_integer_path() {
        let parser = ParserConfig::Json {
            count_path: Some("summary.total".into()),
            minimum: 1,
        };
        assert_eq!(
            parse_result_count(r#"{"summary":{"total":2}}"#, &parser).unwrap(),
            (2, 1)
        );
    }

    #[test]
    fn rejects_invalid_explicit_path_without_auto_discovery_fallback() {
        let parser = ParserConfig::Json {
            count_path: Some("summary.missing".into()),
            minimum: 1,
        };
        assert!(parse_result_count(r#"{"results":[{},{}]}"#, &parser).is_err());
        let parser = ParserConfig::Json {
            count_path: Some("results.".into()),
            minimum: 1,
        };
        assert!(parse_result_count(r#"{"results":[{}]}"#, &parser).is_err());

        let parser = ParserConfig::Json {
            count_path: Some("summary.total".into()),
            minimum: 1,
        };
        assert!(parse_result_count(r#"{"summary":{"total":-1},"results":[{}]}"#, &parser).is_err());
        assert!(
            parse_result_count(r#"{"summary":{"total":1.5},"results":[{}]}"#, &parser).is_err()
        );
        assert!(
            parse_result_count(r#"{"summary":{"total":true},"results":[{}]}"#, &parser).is_err()
        );
    }

    #[test]
    fn auto_discovers_root_and_wrapped_supported_result_arrays() {
        let parser = ParserConfig::Json {
            count_path: None,
            minimum: 1,
        };
        assert_eq!(parse_result_count(r#"[{},{}]"#, &parser).unwrap(), (2, 1));
        assert_eq!(
            parse_result_count(r#"{"suite":{"results":[{},{}]}}"#, &parser).unwrap(),
            (2, 1)
        );
        assert_eq!(
            parse_result_count(r#"{"testCases":[{}]}"#, &parser).unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn auto_discovery_rejects_error_objects_and_unrelated_nested_arrays() {
        let parser = ParserConfig::Json {
            count_path: None,
            minimum: 1,
        };
        assert!(parse_result_count(
            r#"{"duration_ms":5000,"status":"error","message":"failed"}"#,
            &parser
        )
        .is_err());
        assert!(parse_result_count(r#"{"metadata":{"attachments":[{},{}]}}"#, &parser).is_err());
        assert!(parse_result_count(
            r#"{"results":[{}],"metadata":{"attachments":[{},{}]}}"#,
            &parser
        )
        .is_ok());
        assert_eq!(
            parse_result_count(r#"{"duration_ms":5000,"results":[]}"#, &parser).unwrap(),
            (0, 1)
        );
    }

    #[test]
    fn auto_discovery_rejects_ambiguous_candidates_even_when_lengths_match() {
        let parser = ParserConfig::Json {
            count_path: None,
            minimum: 1,
        };
        assert!(parse_result_count(r#"{"results":[{}],"testcases":[{}]}"#, &parser).is_err());
        assert!(
            parse_result_count(r#"{"results":[{}],"suite":{"testResults":[{}]}}"#, &parser)
                .is_err()
        );
    }

    #[test]
    fn auto_discovery_rejects_non_array_supported_fields() {
        let parser = ParserConfig::Json {
            count_path: None,
            minimum: 1,
        };
        assert!(parse_result_count(r#"{"results":2}"#, &parser).is_err());
        assert!(parse_result_count(r#"{"results":{}}"#, &parser).is_err());
        assert!(parse_result_count(r#"{"results":null}"#, &parser).is_err());
    }

    #[test]
    fn malformed_standard_results_fail_closed() {
        let parser = ParserConfig::Junit { minimum: 1 };
        assert!(parse_result_count("<testsuite><testcase>", &parser).is_err());
    }

    #[test]
    fn ambiguous_standard_results_fail_closed() {
        let junit = ParserConfig::Junit { minimum: 1 };
        assert!(parse_result_count("<testsuite/><testsuite/>", &junit).is_err());
        assert!(parse_result_count("<not-a-suite><testcase/></not-a-suite>", &junit).is_err());
        assert!(parse_result_count("<testsuite/>trailing", &junit).is_err());
        assert!(parse_result_count("<testsuite/><![CDATA[trailing]]>", &junit).is_err());
        assert!(parse_result_count("<testsuite/><?xml version=\"1.0\"?>", &junit).is_err());
        assert!(parse_result_count("", &junit).is_err());

        let trx = ParserConfig::Trx { minimum: 1 };
        assert!(parse_result_count("<testsuite/>", &trx).is_err());
    }

    #[test]
    fn shard_merge_rejects_missing_and_duplicate_inputs() {
        let first = ShardResult {
            shard_index: 0,
            shard_total: 2,
            test_ids: vec!["a".into()],
        };
        assert!(merge_shards(std::slice::from_ref(&first), 2)
            .unwrap_err()
            .to_string()
            .contains("missing shard"));
        let duplicate = ShardResult {
            shard_index: 1,
            shard_total: 2,
            test_ids: vec!["a".into()],
        };
        assert!(merge_shards(&[first, duplicate], 2)
            .unwrap_err()
            .to_string()
            .contains("duplicate test identity"));
    }

    #[test]
    fn shard_merge_is_deterministic() {
        let results = vec![
            ShardResult {
                shard_index: 1,
                shard_total: 2,
                test_ids: vec!["b".into()],
            },
            ShardResult {
                shard_index: 0,
                shard_total: 2,
                test_ids: vec!["a".into()],
            },
        ];
        assert_eq!(merge_shards(&results, 2).unwrap(), vec!["a", "b"]);
    }
}
