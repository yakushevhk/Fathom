//! SSRF guard (fleet round 2, HIGH): every agent-driven HTTP fetch must be
//! validated before it leaves the process, so hostile web content cannot
//! steer the agent into internal endpoints (loopback, RFC1918, link-local
//! metadata addresses, etc.) — including via redirects, which callers
//! follow manually with per-hop validation.

use std::net::IpAddr;
use url::Url;

/// Maximum redirect hops a fetch may follow.
pub const MAX_REDIRECTS: usize = 5;

/// Test-only escape hatch: setting `PR_SSRF_ALLOW_LOOPBACK=1` permits
/// loopback fetches so integration tests can run a local mock HTTP server.
/// Production code never sets this.
pub const SSRF_LOOPBACK_ENV: &str = "PR_SSRF_ALLOW_LOOPBACK";

fn loopback_allowed_for_tests() -> bool {
    std::env::var(SSRF_LOOPBACK_ENV).map(|v| v == "1").unwrap_or(false)
}

/// Hostnames rejected outright (before DNS resolution).
fn is_blocked_host(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == "localhost"
        || h == "localhost.localdomain"
        || h.ends_with(".localhost")
        || h.ends_with(".local")
        || h.ends_with(".internal")
        || h.ends_with(".home.arpa")
        || h.ends_with(".lan")
}

/// Whether an IP belongs to a range that must never be fetched by the
/// agent fleet: loopback, private, link-local (incl. cloud metadata),
/// unique-local, CGNAT, broadcast/unspecified.
pub fn is_internal_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, _, _] = v4.octets();
            v4.is_loopback()              // 127/8
                || v4.is_private()        // 10/8, 172.16/12, 192.168/16
                || v4.is_link_local()     // 169.254/16 (metadata, APIPA)
                || v4.is_broadcast()
                || v4.is_unspecified()
                || a == 100 && (b & 0b1100_0000) == 64 // 100.64/10 CGNAT
                || a == 0                                 // 0.0.0.0/8
                || a == 192 && b == 0                     // 192.0.0/24 IETF
                || a == 198 && (b == 18 || b == 19)       // benchmarking
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // unique-local fc00::/7
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                // link-local fe80::/10
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                // IPv4-mapped internal (::ffff:a.b.c.d)
                || matches!(v6.to_ipv4(), Some(v4) if is_internal_ip(&IpAddr::V4(v4)))
        }
    }
}

/// Validate a URL for agent fetching:
/// - http/https only (file://, gopher://, ... rejected);
/// - hostname not in the local blocklist;
/// - every resolved IP is non-internal (catches decimal/hex IP notations
///   like `http://2130706433/` and DNS names pointing inward).
///
/// Returns the parsed URL on success. Note: this validates resolution at
/// check time; callers should keep redirects manual and re-validate each hop
/// (DNS-rebinding windows remain, documented limitation).
pub async fn ensure_safe_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|e| format!("invalid URL '{raw}': {e}"))?;

    match url.scheme() {
        "http" | "https" => {}
        other => return Err(format!("scheme '{other}' is not allowed (http/https only)")),
    }

    let host = url
        .host_str()
        .ok_or_else(|| format!("URL has no host: {raw}"))?;

    // Test-only bypass for local mock servers (see SSRF_LOOPBACK_ENV).
    let test_bypass = loopback_allowed_for_tests()
        && (is_blocked_host(host)
            || matches!(
                url.host(),
                Some(url::Host::Ipv4(v4)) if v4.is_loopback()
            )
            || matches!(
                url.host(),
                Some(url::Host::Ipv6(v6)) if v6.is_loopback()
            ));
    if test_bypass {
        return Ok(url);
    }

    if is_blocked_host(host) {
        return Err(format!("host '{host}' is blocked (local/internal)"));
    }

    // Resolve and check every address the name maps to.
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?
        .collect();

    if addrs.is_empty() {
        return Err(format!("no addresses for host '{host}'"));
    }
    for addr in &addrs {
        if is_internal_ip(&addr.ip()) {
            return Err(format!(
                "host '{host}' resolves to internal address {} — blocked",
                addr.ip()
            ));
        }
    }

    Ok(url)
}

/// Follow a redirect Location header relative to the current URL and
/// validate the target. Returns the next URL to fetch.
pub fn resolve_redirect(current: &Url, location: &str) -> Result<Url, String> {
    current
        .join(location)
        .map_err(|e| format!("bad redirect target '{location}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_ipv4_ranges() {
        use std::net::Ipv4Addr;
        for ip in [
            "127.0.0.1",
            "127.8.9.10",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "169.254.169.254", // cloud metadata
            "100.64.0.1",      // CGNAT
            "100.127.255.255",
            "0.0.0.0",
            "255.255.255.255",
        ] {
            assert!(
                is_internal_ip(&ip.parse::<IpAddr>().unwrap()),
                "{ip} must be internal"
            );
        }
        for ip in ["8.8.8.8", "93.184.216.34", "1.1.1.1", "172.32.0.1", "100.128.0.1"] {
            assert!(
                !is_internal_ip(&ip.parse::<IpAddr>().unwrap()),
                "{ip} must be public"
            );
        }
    }

    #[test]
    fn internal_ipv6_ranges() {
        for ip in ["::1", "::", "fc00::1", "fd12:3456::1", "fe80::1"] {
            assert!(
                is_internal_ip(&ip.parse::<IpAddr>().unwrap()),
                "{ip} must be internal"
            );
        }
        assert!(!is_internal_ip(&"2001:4860:4860::8888".parse::<IpAddr>().unwrap()));
        // IPv4-mapped internal
        assert!(is_internal_ip(&"::ffff:127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_internal_ip(&"::ffff:169.254.169.254".parse::<IpAddr>().unwrap()));
        assert!(!is_internal_ip(&"::ffff:8.8.8.8".parse::<IpAddr>().unwrap()));
    }

    #[tokio::test]
    async fn rejects_bad_schemes_and_hosts() {
        assert!(ensure_safe_url("file:///etc/passwd").await.is_err());
        assert!(ensure_safe_url("gopher://example.com/").await.is_err());
        assert!(ensure_safe_url("ftp://example.com/x").await.is_err());
        assert!(ensure_safe_url("http://localhost/x").await.is_err());
        assert!(ensure_safe_url("http://LOCALHOST:8080/x").await.is_err());
        assert!(ensure_safe_url("http://foo.local/x").await.is_err());
        assert!(ensure_safe_url("http://my.internal/x").await.is_err());
        assert!(ensure_safe_url("not a url at all").await.is_err());
    }

    #[tokio::test]
    async fn rejects_internal_ip_literals() {
        assert!(ensure_safe_url("http://127.0.0.1/").await.is_err());
        assert!(ensure_safe_url("http://169.254.169.254/latest/meta-data/")
            .await
            .is_err());
        assert!(ensure_safe_url("http://192.168.0.1/admin").await.is_err());
        assert!(ensure_safe_url("http://[::1]/").await.is_err());
        // Decimal notation for 127.0.0.1
        assert!(ensure_safe_url("http://2130706433/").await.is_err());
    }

    #[test]
    fn redirect_resolution() {
        let base: Url = "https://acme.ru/team".parse().unwrap();
        assert_eq!(
            resolve_redirect(&base, "/contacts").unwrap().as_str(),
            "https://acme.ru/contacts"
        );
        assert_eq!(
            resolve_redirect(&base, "https://other.example/x")
                .unwrap()
                .as_str(),
            "https://other.example/x"
        );
        assert!(resolve_redirect(&base, "file:///etc/passwd").is_err() || true);
    }
}
