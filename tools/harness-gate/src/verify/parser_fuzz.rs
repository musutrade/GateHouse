//! Bounded deterministic mutation smoke test, not coverage-guided fuzzing.
//! Reproduce with nextest's `test(bounded_result_parser_mutations)` filter.
use super::{parse_result_count, ParserConfig};

const SEEDS: [u64; 2] = [0x59a1_2026_1015, 0x9e37_79b9_7f4a_7c15];
const CASES_PER_SEED: usize = 256;
const MAX_BYTES: usize = 4096;
const CORPUS: &[&str] = &[
    r#"{"results":[{},{}]}"#,
    r#"{"summary":{"total":2},"wrapper":{"tests":[{}]}}"#,
    r#"{"results":[],"tests":[]}"#,
    r#"{"summary":{"total":18446744073709551616}}"#,
    r#"{"summary":{"total":-1},"results":"not-an-array"}"#,
    "[null, true, \"边界\", {}]",
    "\x1b[31m{\"results\":[]}\x1b[0m",
    "<testsuite><testcase/><testcase/></testsuite>",
    "<j:testsuites xmlns:j=\"urn:junit\"><j:testsuite><j:testcase/></j:testsuite></j:testsuites>",
    "<testsuite><!-- <testcase/> --><![CDATA[<testcase/>]]></testsuite>",
    "<TestRun><Results><UnitTestResult testId=\"a\"/></Results></TestRun>",
    "<!DOCTYPE testsuite [<!ENTITY x SYSTEM 'file:///nonexistent'>]><testsuite>&x;</testsuite>",
    "<testsuite/><testsuite/>",
    "",
];

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn mutate(input: &str, state: &mut u64) -> String {
    const TOKENS: &[char] = &[
        '{', '}', '[', ']', '<', '>', '/', '"', '\'', ':', ',', '=', '&', ';', '0', '9', '-', '+',
        '\n', '\0', '\u{1b}', 'é', '界', '\\', 'a',
    ];
    let mut chars: Vec<char> = input.chars().collect();
    let changes = 1 + (next(state) % 16) as usize;
    for _ in 0..changes {
        let at = (next(state) % (chars.len() + 1) as u64) as usize;
        let token = TOKENS[(next(state) % TOKENS.len() as u64) as usize];
        match next(state) % 3 {
            0 if at < chars.len() => {
                chars.remove(at);
            }
            1 if at < chars.len() => chars[at] = token,
            _ => chars.insert(at, token),
        }
    }
    let mut result: String = chars.into_iter().collect();
    if result.len() > MAX_BYTES {
        let mut end = MAX_BYTES;
        while !result.is_char_boundary(end) {
            end -= 1;
        }
        result.truncate(end);
    }
    result
}

#[test]
fn bounded_result_parser_mutations() {
    let parsers = [
        ParserConfig::Json {
            count_path: None,
            minimum: 7,
        },
        ParserConfig::Json {
            count_path: Some("summary.total".into()),
            minimum: 7,
        },
        ParserConfig::Junit { minimum: 7 },
        ParserConfig::Trx { minimum: 7 },
    ];
    // Count-bearing valid inputs remain a semantic oracle, rather than merely
    // accepting every error as a successful fuzz execution.
    for count in 0..=32 {
        let json = format!("{{\"results\":[{}]}}", vec!["{}"; count].join(","));
        let explicit = format!("{{\"summary\":{{\"total\":{count}}}}}");
        let junit = format!("<testsuite>{}</testsuite>", "<testcase/>".repeat(count));
        let trx = format!("<TestRun>{}</TestRun>", "<UnitTestResult/>".repeat(count));
        for (parser, input) in parsers.iter().zip([json, explicit, junit, trx]) {
            assert_eq!(parse_result_count(&input, parser).unwrap(), (count, 7));
        }
    }
    let mut calls = 0;
    for seed in SEEDS {
        let mut state = seed;
        for case in 0..CASES_PER_SEED {
            let input = mutate(CORPUS[case % CORPUS.len()], &mut state);
            assert!(input.len() <= MAX_BYTES);
            for (target, parser) in parsers.iter().enumerate() {
                let result = std::panic::catch_unwind(|| parse_result_count(&input, parser));
                match result {
                    Ok(Ok((_, minimum))) => assert_eq!(minimum, 7),
                    Ok(Err(_)) => {} // malformed inputs may be rejected
                    Err(_) => panic!("seed={seed:#x} case={case} target={target} input={input:?}"),
                }
                calls += 1;
            }
        }
    }
    assert_eq!(calls, SEEDS.len() * CASES_PER_SEED * parsers.len());
    eprintln!("bounded parser mutations: {calls} calls, max {MAX_BYTES} bytes, seeds={SEEDS:x?}");
}
