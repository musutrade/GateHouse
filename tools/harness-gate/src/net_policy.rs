use std::net::IpAddr;

/// Normalize a host for exact allowlist comparison. A trailing DNS root dot is
/// insignificant, while wildcard entries remain invalid by design.
pub(crate) fn normalize_host(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

pub(crate) fn valid_allowlist_host(host: &str) -> bool {
    let normalized = normalize_host(host);
    !normalized.is_empty()
        && normalized == host.trim_end_matches('.').to_ascii_lowercase()
        && !normalized.contains('*')
        && !normalized.chars().any(|character| {
            character.is_ascii_whitespace()
                || character.is_ascii_control()
                || matches!(character, '/' | '\\' | '@' | '?')
        })
        && (normalized.parse::<IpAddr>().is_ok()
            || normalized
                .split('.')
                .all(|label| !label.is_empty() && label.bytes().all(is_host_byte)))
}

fn is_host_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

#[cfg(test)]
mod tests {
    use super::{normalize_host, valid_allowlist_host};

    #[test]
    fn normalizes_exact_hosts_without_wildcards() {
        assert_eq!(normalize_host("Hooks.Example.TEST."), "hooks.example.test");
        assert!(valid_allowlist_host("hooks.example.test"));
        assert!(valid_allowlist_host("127.0.0.1"));
        assert!(!valid_allowlist_host("*.example.test"));
        assert!(!valid_allowlist_host("https://example.test"));
    }
}
