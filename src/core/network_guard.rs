use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use url::Url;

use crate::core::error::{Result, SerpError};

/// Check whether an IP address is considered public and safe to contact.
/// Rejects loopback, private RFC 1918, link-local, carrier-grade NAT (100.64.0.0/10),
/// multicast, unspecified, and non-routable ranges.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => {
            // Check if this is an IPv4-mapped IPv6 address (::ffff:192.0.2.1)
            if let Some(mapped_v4) = v6.to_ipv4_mapped() {
                return is_public_ipv4(mapped_v4);
            }
            is_public_ipv6(v6)
        }
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();

    // Loopback (127.0.0.0/8)
    if ip.is_loopback() {
        return false;
    }

    // Unspecified (0.0.0.0)
    if ip.is_unspecified() {
        return false;
    }

    // Broadcast (255.255.255.255)
    if ip.is_broadcast() {
        return false;
    }

    // Multicast (224.0.0.0/4)
    if ip.is_multicast() {
        return false;
    }

    // Link-local (169.254.0.0/16)
    if ip.is_link_local() {
        return false;
    }

    // Private RFC 1918 (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
    if ip.is_private() {
        return false;
    }

    // Carrier-grade NAT RFC 6598 (100.64.0.0/10)
    // Range: 100.64.0.0 to 100.127.255.255
    if octets[0] == 100 && (octets[1] & 0xC0) == 64 {
        return false;
    }

    // Documentation RFC 5737: 192.0.2.0/24 (TEST-NET-1), 198.51.100.0/24 (TEST-NET-2), 203.0.113.0/24 (TEST-NET-3)
    if (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
    {
        return false;
    }

    // Benchmarking RFC 2544 (198.18.0.0/15)
    if octets[0] == 198 && (octets[1] & 0xFE) == 18 {
        return false;
    }

    true
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();

    // Loopback (::1)
    if ip.is_loopback() {
        return false;
    }

    // Unspecified (::)
    if ip.is_unspecified() {
        return false;
    }

    // Multicast (ff00::/8)
    if ip.is_multicast() {
        return false;
    }

    // Unique Local Address RFC 4193 (fc00::/7)
    if (segments[0] & 0xfe00) == 0xfc00 {
        return false;
    }

    // Link-local unicast RFC 4291 (fe80::/10)
    if (segments[0] & 0xffc0) == 0xfe80 {
        return false;
    }

    true
}

/// Validates that an HTTP(S) URL points to a public, safe endpoint (SSRF protection).
/// Verifies the scheme is http/https, host is non-empty, and all resolved IP addresses
/// are public.
pub async fn validate_public_url(raw_url: &str) -> Result<Url> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return Err(SerpError::InvalidParam("URL cannot be empty".to_string()));
    }

    let parsed = Url::parse(trimmed).map_err(|e| {
        SerpError::InvalidParam(format!("Invalid URL format: {}", e))
    })?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(SerpError::InvalidParam(format!(
            "Unsupported URL scheme '{}': only http and https are allowed",
            parsed.scheme()
        )));
    }

    let host = match parsed.host_str() {
        Some(h) if !h.is_empty() => h,
        _ => return Err(SerpError::InvalidParam("URL host is required".to_string())),
    };

    // If host is a literal IP address
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !is_public_ip(ip) {
            return Err(SerpError::Blocked(format!(
                "Target host '{}' resolves to non-public IP: {}",
                host, ip
            )));
        }
        return Ok(parsed);
    }

    // Resolve domain host
    let port = parsed.port_or_known_default().unwrap_or(80);
    let socket_addr_str = format!("{}:{}", host, port);

    match tokio::net::lookup_host(&socket_addr_str).await {
        Ok(addrs) => {
            let mut resolved_any = false;
            for addr in addrs {
                resolved_any = true;
                if !is_public_ip(addr.ip()) {
                    return Err(SerpError::Blocked(format!(
                        "Target host '{}' resolves to non-public IP: {}",
                        host, addr.ip()
                    )));
                }
            }
            if !resolved_any {
                return Err(SerpError::InvalidParam(format!(
                    "Target host '{}' resolved to no IP addresses",
                    host
                )));
            }
        }
        Err(e) => {
            return Err(SerpError::InvalidParam(format!(
                "Could not resolve host '{}': {}",
                host, e
            )));
        }
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_public_ipv4() {
        // Loopback
        assert!(!is_public_ip("127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("127.0.1.1".parse().unwrap()));

        // Private RFC 1918
        assert!(!is_public_ip("10.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("172.16.0.1".parse().unwrap()));
        assert!(!is_public_ip("172.31.255.254".parse().unwrap()));
        assert!(!is_public_ip("192.168.1.1".parse().unwrap()));

        // Carrier-grade NAT
        assert!(!is_public_ip("100.64.0.1".parse().unwrap()));
        assert!(!is_public_ip("100.127.255.254".parse().unwrap()));
        // Outside carrier-grade NAT
        assert!(is_public_ip("100.128.0.1".parse().unwrap()));

        // Link-local
        assert!(!is_public_ip("169.254.1.1".parse().unwrap()));

        // Public IPs
        assert!(is_public_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
        assert!(is_public_ip("142.250.190.46".parse().unwrap()));
    }

    #[test]
    fn test_is_public_ipv6() {
        // Loopback
        assert!(!is_public_ip("::1".parse().unwrap()));

        // Unspecified
        assert!(!is_public_ip("::".parse().unwrap()));

        // ULA private
        assert!(!is_public_ip("fc00::1".parse().unwrap()));
        assert!(!is_public_ip("fd12:3456:789a:1::1".parse().unwrap()));

        // Link-local
        assert!(!is_public_ip("fe80::1".parse().unwrap()));

        // IPv4 mapped private
        assert!(!is_public_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("::ffff:10.0.0.1".parse().unwrap()));

        // Public IPv6
        assert!(is_public_ip("2001:4860:4860::8888".parse().unwrap()));
    }

    #[tokio::test]
    async fn test_validate_public_url_rejections() {
        assert!(validate_public_url("http://127.0.0.1/").await.is_err());
        assert!(validate_public_url("https://10.0.0.1/admin").await.is_err());
        assert!(validate_public_url("ftp://example.com").await.is_err());
        assert!(validate_public_url("").await.is_err());
    }
}
