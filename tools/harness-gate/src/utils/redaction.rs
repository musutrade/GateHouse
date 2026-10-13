//! Shared redaction for every text boundary that can leave an invocation.

use regex::Regex;
use std::{borrow::Cow, sync::OnceLock};

pub(crate) const REDACTION_TEXT_LIMIT: usize = 16 * 1024 * 1024;

/// Replace credential-bearing values while preserving enough surrounding
/// context for a human to identify the failing rule or operation.
pub(crate) fn redact_text(input: &str) -> String {
    redact_text_cow(input).into_owned()
}

// Borrow the original input until a pattern actually replaces text. Public
// callers keep the existing String return contract and regex traversal order.
fn redact_text_cow(input: &str) -> Cow<'_, str> {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // Run before other patterns can change string escape boundaries.
            // A backslash consumes the next character, so only an unescaped
            // quote ends a JSON string (including runs of odd/even backslashes).
            (Regex::new(r#"(?i)("(?:\\.|[^"\\])*(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|password|passwd|secret|client[_-]?secret)(?:\\.|[^"\\])*"\s*:\s*)"(?:\\.|[^"\\])*""#)
                .expect("json secret redaction regex"), "${1}\"[REDACTED]\""),
            (Regex::new(r"(?is)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)")
                .expect("private key redaction regex"), "[REDACTED]"),
            // Recognize only explicit log fields before a header, not arbitrary
            // prose. Keep indentation, bracketed fields, ISO timestamps, levels
            // and curl's direction marker; redact the entire header and attributes.
            (Regex::new(r"(?im)^([ \t]*(?:(?:\[[^\]\r\n]*\]|\d{4}-\d{2}-\d{2}[T ][0-9:.]+(?:Z|[+-]\d{2}:?\d{2})?|(?:TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL)\b:?|[<>])[ \t]+)*)(?:authorization|proxy-authorization|cookie|set-cookie|x-api-key|x-auth-token)[ \t]*:[^\r\n]*")
                .expect("header redaction regex"), "${1}[REDACTED]"),
            (Regex::new(r#"(?i)\b(?:postgres(?:ql)?|mysql|redis|mongodb(?:\+srv)?)://[^\s<>'\"]+"#)
                .expect("connection string redaction regex"), "[REDACTED]"),
            (Regex::new(r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]+")
                .expect("authorization redaction regex"), "[REDACTED]"),
            (Regex::new(r##"(?i)\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|password|passwd|secret|client[_-]?secret)\b\s*[:=]\s*(?:"(?:\\.|[^"\\\r\n])*"?|'(?:\\.|[^'\\\r\n])*'?|[^\s"'`,;}]+)"##)
                .expect("assignment redaction regex"), "[REDACTED]"),
        ]
    });
    let mut text = Cow::Borrowed(input);
    for (pattern, replacement) in patterns {
        let replaced = match pattern.replace_all(text.as_ref(), *replacement) {
            Cow::Owned(replaced) => Some(replaced),
            Cow::Borrowed(_) => None,
        };
        if let Some(replaced) = replaced {
            text = Cow::Owned(replaced);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::redact_text;

    #[test]
    fn removes_common_credential_shapes() {
        let text = concat!(
            "Authorization: Bearer opaque-token\n",
            "cookie: session=opaque-cookie\n",
            "DATABASE_URL=postgres://user:opaque-pass@db.example.test/app\n",
            "password=opaque-value\n",
            "{\"api_key\":\"opaque-json\"}\n",
            "-----BEGIN PRIVATE KEY-----\nopaque-material\n-----END PRIVATE KEY-----"
        );
        let redacted = redact_text(text);
        for secret in [
            "opaque-token",
            "opaque-cookie",
            "opaque-pass@db.example.test",
            "opaque-value",
            "opaque-json",
            "opaque-material",
        ] {
            assert!(!redacted.contains(secret), "secret leaked: {secret}");
        }
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn json_strings_redact_escaped_quotes_backslashes_and_unicode_in_logs() {
        for key in ["password", "token", "secret", "client_secret", "api_key"] {
            for count in 0..=6 {
                for suffix in ["\"SYNTHETIC_SUFFIX", "SYNTHETIC_SUFFIX"] {
                    // URI and auth replacements must not alter JSON boundaries
                    // before the complete credential value has been consumed.
                    let secret = format!(
                        "postgres://u:p@db/SYNTHETIC_PREFIX Bearer opaque {}{suffix}",
                        "\\".repeat(count)
                    );
                    let json =
                        serde_json::json!({key: secret, "context": "keep-context", "count": 7});
                    let input = format!("INFO request {json}\n{json}\n");
                    let redacted = redact_text(&input);
                    assert!(!redacted.contains("SYNTHETIC_"), "leaked: {redacted}");
                    assert!(redacted.starts_with("INFO request "));
                    for line in redacted.lines() {
                        let json = line.strip_prefix("INFO request ").unwrap_or(line);
                        let value: serde_json::Value = serde_json::from_str(json).unwrap();
                        assert_eq!(value[key], "[REDACTED]");
                        assert_eq!(value["context"], "keep-context");
                        assert_eq!(value["count"], 7);
                    }
                }
            }
        }
        let input = r#"{"password":"prefix\"QUOTE_SUFFIX","token":"prefix\\","secret":"\u0053UNICODE_SUFFIX","context":"public\"quote","count":9}"#;
        let redacted = redact_text(input);
        let value: serde_json::Value = serde_json::from_str(&redacted).unwrap();
        for key in ["password", "token", "secret"] {
            assert_eq!(value[key], "[REDACTED]");
        }
        assert_eq!(value["context"], "public\"quote");
        assert_eq!(value["count"], 9);
        assert!(!redacted.contains("SUFFIX"));
    }

    #[test]
    fn quoted_assignment_values_are_redacted_to_the_closing_quote() {
        for (input, expected) in [
            (
                r#"password="my secret phrase" user=alice"#,
                "[REDACTED] user=alice",
            ),
            ("token: 'quoted value here', next", "[REDACTED], next"),
            (
                r#"secret="a \"quoted\" secret phrase" tail"#,
                "[REDACTED] tail",
            ),
            (r"api_key='it\'s secret' tail", "[REDACTED] tail"),
            (
                "password=\"unclosed secret phrase\npublic-next",
                "[REDACTED]\npublic-next",
            ),
            (
                r#"user=alice password="first secret" region=eu token='second secret' ok"#,
                "user=alice [REDACTED] region=eu [REDACTED] ok",
            ),
            ("password=plain-value, public", "[REDACTED], public"),
            (r#"password="" public"#, "[REDACTED] public"),
        ] {
            let redacted = redact_text(input);
            assert_eq!(redacted, expected, "input: {input}");
            assert!(!redacted.contains("secret phrase") && !redacted.contains("second"));
        }
        let public = r#"message="no credentials here" status=ok"#;
        assert_eq!(redact_text(public), public);
    }

    #[test]
    fn private_keys_redact_closed_blocks_and_unclosed_blocks_to_eof() {
        for label in [
            "PRIVATE KEY",
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "ENCRYPTED PRIVATE KEY",
        ] {
            for newline in ["\n", "\r\n"] {
                let begin = format!(
                    "public-before{newline}-----BEGIN {label}-----{newline}SYNTHETIC_KEY{newline}"
                );
                assert_eq!(
                    redact_text(&format!("{begin}SYNTHETIC_EOF")),
                    format!("public-before{newline}[REDACTED]")
                );
                let closed = format!("{begin}-----END {label}-----{newline}public-after");
                assert_eq!(
                    redact_text(&closed),
                    format!("public-before{newline}[REDACTED]{newline}public-after")
                );
            }
        }
    }

    #[test]
    fn headers_redact_complete_values_at_explicit_log_boundaries() {
        for prefix in [
            "",
            "  ",
            "\t",
            "[INFO] ",
            "[2026-10-08T12:00:00Z] [worker] ",
            "INFO: ",
            "2026-10-08 12:00:00.123 WARN ",
            "2026-10-08T12:00:00+00:00 DEBUG ",
            "> ",
            "  < ",
        ] {
            for header in [
                "Cookie",
                "Set-Cookie",
                "Authorization",
                "Proxy-Authorization",
                "X-Api-Key",
                "X-Auth-Token",
            ] {
                let input = format!("{prefix}{header}: session=SYNTHETIC_COOKIE; another=SYNTHETIC_OTHER; Path=/private path; HttpOnly\r\npublic-after");
                assert_eq!(
                    redact_text(&input),
                    format!("{prefix}[REDACTED]\r\npublic-after")
                );
            }
        }
        for public in [
            "A guide mentions cookie: a public example.",
            "INFO a guide mentions Set-Cookie: public example",
            "[guide] a cookie: public example",
            "Cookie policy: public",
            r#"{"message":"Cookie: public example","count":3}"#,
        ] {
            assert_eq!(redact_text(public), public);
        }
    }
}

#[cfg(test)]
mod cow_parity_tests {
    use super::*;

    #[test]
    fn cow_no_hit_borrows_and_mixed_replacements_match_frozen_g_bytes() {
        let public = "INFO no credentials: public facts é😀\n".repeat(8192);
        assert!(matches!(redact_text_cow(&public), Cow::Borrowed(_)));
        assert_eq!(redact_text(&public), public);
        let mut cases = vec![
            String::new(),
            public.clone(),
            "Explain Authorization: Bearer opaque in prose\n".into(),
            "[worker] INFO Cookie: x=first; y=second; Path=/\n".into(),
            "password=opaque redis://u:p@db Bearer opaque\n".into(),
            "-----BEGIN PRIVATE KEY-----\nunclosed-to-eof".into(),
            "password=\"quoted secret phrase\" token='second one' public\n".into(),
        ];
        for count in 0..=6 {
            let secret = format!(
                "postgres://u:p@db/ Bearer opaque {}\"suffix é😀",
                "\\".repeat(count)
            );
            cases.push(serde_json::json!({"token":secret,"context":"public"}).to_string());
        }
        cases.push(format!(
            "{public}\n[worker] Authorization: Basic opaque; token=second\n{public}"
        ));
        for input in cases {
            assert_eq!(redact_text_cow(&input).as_ref(), redact_text_g(&input));
            assert_eq!(
                redact_text(&input).as_bytes(),
                redact_text_g(&input).as_bytes()
            );
        }
        assert!(matches!(redact_text_cow("token=opaque"), Cow::Owned(_)));
    }

    // Fold-based text oracle; patterns and ordering mirror `redact_text_cow`
    // byte-for-byte (the assignment rule includes the #310 quoted-value form).
    fn redact_text_g(input: &str) -> String {
        static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
        let patterns = PATTERNS.get_or_init(|| {
        vec![
            // Run before other patterns can change string escape boundaries.
            // A backslash consumes the next character, so only an unescaped
            // quote ends a JSON string (including runs of odd/even backslashes).
            (Regex::new(r#"(?i)("(?:\\.|[^"\\])*(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|password|passwd|secret|client[_-]?secret)(?:\\.|[^"\\])*"\s*:\s*)"(?:\\.|[^"\\])*""#)
                .expect("json secret redaction regex"), "${1}\"[REDACTED]\""),
            (Regex::new(r"(?is)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)")
                .expect("private key redaction regex"), "[REDACTED]"),
            // Recognize only explicit log fields before a header, not arbitrary
            // prose. Keep indentation, bracketed fields, ISO timestamps, levels
            // and curl's direction marker; redact the entire header and attributes.
            (Regex::new(r"(?im)^([ \t]*(?:(?:\[[^\]\r\n]*\]|\d{4}-\d{2}-\d{2}[T ][0-9:.]+(?:Z|[+-]\d{2}:?\d{2})?|(?:TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL)\b:?|[<>])[ \t]+)*)(?:authorization|proxy-authorization|cookie|set-cookie|x-api-key|x-auth-token)[ \t]*:[^\r\n]*")
                .expect("header redaction regex"), "${1}[REDACTED]"),
            (Regex::new(r#"(?i)\b(?:postgres(?:ql)?|mysql|redis|mongodb(?:\+srv)?)://[^\s<>'\"]+"#)
                .expect("connection string redaction regex"), "[REDACTED]"),
            (Regex::new(r"(?i)\b(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]+")
                .expect("authorization redaction regex"), "[REDACTED]"),
            (Regex::new(r##"(?i)\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|password|passwd|secret|client[_-]?secret)\b\s*[:=]\s*(?:"(?:\\.|[^"\\\r\n])*"?|'(?:\\.|[^'\\\r\n])*'?|[^\s"'`,;}]+)"##)
                .expect("assignment redaction regex"), "[REDACTED]"),
        ]
    });
        patterns
            .iter()
            .fold(input.to_string(), |text, (pattern, replacement)| {
                pattern.replace_all(&text, *replacement).into_owned()
            })
    }
}
