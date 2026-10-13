use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

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

/// Non-public ranges are denied for every resolved address. This covers
/// private, shared (CGNAT), benchmarking, documentation and reserved IPv4
/// space, IPv6 unique-local/link-local/documentation space, and every IPv6
/// form that embeds an IPv4 address (mapped, compatible, NAT64, 6to4).
pub(crate) fn is_local_only(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_local_ipv4(address),
        IpAddr::V6(address) => is_local_ipv6(address),
    }
}

fn is_local_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_multicast()
        || a == 0
        // 100.64.0.0/10 shared address space (CGNAT).
        || (a == 100 && (b & 0xc0) == 64)
        // 192.0.0.0/24 IETF protocol assignments.
        || (a == 192 && b == 0 && c == 0)
        // 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24 documentation.
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        // 198.18.0.0/15 benchmarking.
        || (a == 198 && (b & 0xfe) == 18)
        // 240.0.0.0/4 reserved.
        || a >= 240
}

fn is_local_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        // fec0::/10 deprecated site-local.
        || (segments[0] & 0xffc0) == 0xfec0
        // 2001:db8::/32 and 3fff::/20 documentation.
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x3fff && (segments[1] & 0xf000) == 0)
        // 100::/64 discard-only.
        || segments[..4] == [0x0100, 0, 0, 0]
        // ::/96 IPv4-compatible (deprecated, never publicly routed).
        || segments[..6] == [0, 0, 0, 0, 0, 0]
        // 64:ff9b:1::/48 local-use NAT64; its embedding is operator-defined.
        || segments[..3] == [0x0064, 0xff9b, 0x0001]
        || embedded_ipv4(address).is_some_and(is_local_ipv4)
}

/// Extract an IPv4 address carried inside an IPv6 address, so translation
/// and transition prefixes cannot reach a local IPv4 target indirectly.
fn embedded_ipv4(address: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = address.segments();
    let low = |high: u16, low: u16| {
        let [a, b] = high.to_be_bytes();
        let [c, d] = low.to_be_bytes();
        Ipv4Addr::new(a, b, c, d)
    };
    if let Some(mapped) = address.to_ipv4_mapped() {
        return Some(mapped);
    }
    match segments {
        // 64:ff9b::/96 well-known NAT64 prefix.
        [0x0064, 0xff9b, 0, 0, 0, 0, high, low_bits] => Some(low(high, low_bits)),
        // 2002::/16 6to4 embeds the IPv4 address in bits 16..48.
        [0x2002, high, low_bits, ..] => Some(low(high, low_bits)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_local_only, normalize_host, valid_allowlist_host};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn normalizes_exact_hosts_without_wildcards() {
        assert_eq!(normalize_host("Hooks.Example.TEST."), "hooks.example.test");
        assert!(valid_allowlist_host("hooks.example.test"));
        assert!(valid_allowlist_host("127.0.0.1"));
        assert!(!valid_allowlist_host("*.example.test"));
        assert!(!valid_allowlist_host("https://example.test"));
    }

    #[test]
    fn rejects_local_address_matrix() {
        for address in [
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V4(Ipv4Addr::BROADCAST),
            IpAddr::V4(Ipv4Addr::new(0, 12, 34, 56)),
            IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("fc00::1".parse().expect("unique local address")),
            IpAddr::V6("fe80::1".parse().expect("link local address")),
            IpAddr::V6("ff00::1".parse().expect("multicast address")),
            IpAddr::V6("::ffff:127.0.0.1".parse().expect("mapped loopback")),
        ] {
            assert!(is_local_only(address), "expected local address: {address}");
        }
        for address in [
            "100.64.0.1",
            "100.127.255.254",
            "192.0.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.19.255.254",
            "198.51.100.1",
            "203.0.113.1",
            "240.0.0.1",
            "255.255.255.254",
            "::ffff:169.254.169.254",
            "64:ff9b::a9fe:a9fe",
            "64:ff9b::169.254.169.254",
            "64:ff9b::10.0.0.1",
            "64:ff9b::127.0.0.1",
            "64:ff9b::100.64.0.1",
            "64:ff9b:1::808:808",
            "::169.254.169.254",
            "::8.8.8.8",
            "2002:a9fe:a9fe::1",
            "2002:7f00:1::1",
            "2001:db8::1",
            "3fff::1",
            "fec0::1",
            "100::1",
        ] {
            let address: IpAddr = address.parse().expect("reserved address");
            assert!(is_local_only(address), "expected local address: {address}");
        }
        for address in [
            "8.8.8.8",
            "1.1.1.1",
            "100.63.255.255",
            "100.128.0.1",
            "198.17.255.255",
            "198.20.0.1",
            "192.0.1.1",
            "2001:4860:4860::8888",
            "2606:4700:4700::1111",
            "64:ff9b::808:808",
            "2002:808:808::1",
        ] {
            let address: IpAddr = address.parse().expect("public address");
            assert!(
                !is_local_only(address),
                "expected public address: {address}"
            );
        }
    }
}
