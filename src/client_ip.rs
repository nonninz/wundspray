//! Client IP resolution behind trusted ingress proxies.

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use axum::http::HeaderMap;
use ipnet::IpNet;

/// Parse `TRUSTED_PROXIES`: comma-separated CIDRs (single IPs become /32 or /128).
///
/// Empty / missing → empty list (never trust forwarding headers).
pub fn parse_trusted_proxies(raw: &str) -> Result<Vec<IpNet>, String> {
    let mut nets = Vec::new();
    for part in raw.split(',') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }
        let net = parse_proxy_token(token)?;
        nets.push(net);
    }
    Ok(nets)
}

fn parse_proxy_token(token: &str) -> Result<IpNet, String> {
    if let Ok(net) = IpNet::from_str(token) {
        return Ok(net);
    }
    if let Ok(ip) = IpAddr::from_str(token) {
        return Ok(IpNet::from(ip));
    }
    Err(format!("invalid TRUSTED_PROXIES entry: {token}"))
}

fn is_trusted(ip: IpAddr, trusted: &[IpNet]) -> bool {
    trusted.iter().any(|net| net.contains(&ip))
}

/// Which input produced the resolved client IP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIpSource {
    Peer,
    Forwarded,
    XForwardedFor,
    XRealIp,
}

impl ClientIpSource {
    /// Stable string for structured logs: `peer` / `forwarded` / `xff` / `x-real-ip`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Peer => "peer",
            Self::Forwarded => "forwarded",
            Self::XForwardedFor => "xff",
            Self::XRealIp => "x-real-ip",
        }
    }
}

/// Result of client-IP resolution, including diagnostics for DEBUG logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIpResolution {
    pub client_ip: IpAddr,
    pub peer_trusted: bool,
    pub source: ClientIpSource,
}

/// Resolve the requestor IP from the TCP peer and optional proxy headers.
///
/// Forwarding headers are only consulted when `peer` is in `trusted`.
/// Prefer `Forwarded`, then `X-Forwarded-For` (strip trailing trusted hops,
/// take the rightmost remaining address), then `X-Real-IP`, else the peer IP.
pub fn resolve_client_ip(
    peer: SocketAddr,
    headers: &HeaderMap,
    trusted: &[IpNet],
) -> ClientIpResolution {
    let peer_ip = peer.ip();
    let peer_trusted = !trusted.is_empty() && is_trusted(peer_ip, trusted);
    if !peer_trusted {
        return ClientIpResolution {
            client_ip: peer_ip,
            peer_trusted: false,
            source: ClientIpSource::Peer,
        };
    }

    if let Some(ip) = client_from_forwarded(headers, trusted) {
        return ClientIpResolution {
            client_ip: ip,
            peer_trusted: true,
            source: ClientIpSource::Forwarded,
        };
    }
    if let Some(ip) = client_from_xff(headers, trusted) {
        return ClientIpResolution {
            client_ip: ip,
            peer_trusted: true,
            source: ClientIpSource::XForwardedFor,
        };
    }
    if let Some(ip) = client_from_x_real_ip(headers) {
        return ClientIpResolution {
            client_ip: ip,
            peer_trusted: true,
            source: ClientIpSource::XRealIp,
        };
    }

    ClientIpResolution {
        client_ip: peer_ip,
        peer_trusted: true,
        source: ClientIpSource::Peer,
    }
}

fn client_from_forwarded(headers: &HeaderMap, trusted: &[IpNet]) -> Option<IpAddr> {
    let value = headers.get("forwarded")?.to_str().ok()?;
    let ips: Vec<IpAddr> = value
        .split(',')
        .filter_map(|element| parse_forwarded_for(element.trim()))
        .collect();
    rightmost_untrusted(&ips, trusted)
}

fn parse_forwarded_for(element: &str) -> Option<IpAddr> {
    for param in element.split(';') {
        let param = param.trim();
        let Some(rest) = param
            .strip_prefix("for=")
            .or_else(|| param.strip_prefix("For="))
            .or_else(|| param.strip_prefix("FOR="))
        else {
            continue;
        };
        return parse_ip_token(rest);
    }
    None
}

fn client_from_xff(headers: &HeaderMap, trusted: &[IpNet]) -> Option<IpAddr> {
    let value = headers.get("x-forwarded-for")?.to_str().ok()?;
    let ips: Vec<IpAddr> = value
        .split(',')
        .filter_map(|part| parse_ip_token(part.trim()))
        .collect();
    rightmost_untrusted(&ips, trusted)
}

fn client_from_x_real_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let value = headers.get("x-real-ip")?.to_str().ok()?;
    parse_ip_token(value.trim())
}

/// Strip trailing trusted hops; return the rightmost remaining address.
fn rightmost_untrusted(ips: &[IpAddr], trusted: &[IpNet]) -> Option<IpAddr> {
    if ips.is_empty() {
        return None;
    }
    let mut end = ips.len();
    while end > 0 && is_trusted(ips[end - 1], trusted) {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    Some(ips[end - 1])
}

fn parse_ip_token(raw: &str) -> Option<IpAddr> {
    let mut s = raw.trim();
    if s.is_empty() {
        return None;
    }
    // Quoted forms: "1.2.3.4" or "[2001:db8::1]"
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s = &s[1..s.len() - 1];
    }
    // Bracketed IPv6 (with optional :port)
    if s.starts_with('[') {
        let end = s.find(']')?;
        return IpAddr::from_str(&s[1..end]).ok();
    }
    // IPv4 with optional :port — prefer full parse, else strip port
    if let Ok(ip) = IpAddr::from_str(s) {
        return Some(ip);
    }
    if let Some((host, _port)) = s.rsplit_once(':') {
        // Only treat as host:port for IPv4 (single colon)
        if !host.contains(':') {
            return IpAddr::from_str(host).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::net::{Ipv4Addr, SocketAddrV4};

    fn peer(ip: &str, port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from_str(ip).unwrap(), port))
    }

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    #[test]
    fn parse_empty_trusted_proxies() {
        assert!(parse_trusted_proxies("").unwrap().is_empty());
        assert!(parse_trusted_proxies("  , , ").unwrap().is_empty());
    }

    #[test]
    fn parse_cidrs_and_single_ips() {
        let nets = parse_trusted_proxies("10.0.0.0/8, 192.168.1.1, ::1").unwrap();
        assert_eq!(nets.len(), 3);
        assert!(nets[0].contains(&IpAddr::from_str("10.1.2.3").unwrap()));
        assert!(nets[1].contains(&IpAddr::from_str("192.168.1.1").unwrap()));
        assert!(!nets[1].contains(&IpAddr::from_str("192.168.1.2").unwrap()));
        assert!(nets[2].contains(&IpAddr::from_str("::1").unwrap()));
    }

    #[test]
    fn empty_trusted_ignores_spoofed_xff() {
        let peer_addr = peer("203.0.113.50", 443);
        let hdrs = headers(&[("x-forwarded-for", "198.51.100.1")]);
        let resolved = resolve_client_ip(peer_addr, &hdrs, &[]);
        assert_eq!(
            resolved.client_ip,
            IpAddr::from_str("203.0.113.50").unwrap()
        );
        assert!(!resolved.peer_trusted);
        assert_eq!(resolved.source, ClientIpSource::Peer);
    }

    #[test]
    fn untrusted_peer_ignores_headers() {
        let trusted = parse_trusted_proxies("10.0.0.0/8").unwrap();
        let peer_addr = peer("203.0.113.50", 443);
        let hdrs = headers(&[
            ("x-forwarded-for", "198.51.100.1"),
            ("x-real-ip", "198.51.100.2"),
        ]);
        let resolved = resolve_client_ip(peer_addr, &hdrs, &trusted);
        assert_eq!(
            resolved.client_ip,
            IpAddr::from_str("203.0.113.50").unwrap()
        );
        assert!(!resolved.peer_trusted);
        assert_eq!(resolved.source, ClientIpSource::Peer);
    }

    #[test]
    fn trusted_peer_xff_chain_strips_trailing_trusted() {
        let trusted = parse_trusted_proxies("10.0.0.0/8, 192.168.0.0/16").unwrap();
        let peer_addr = peer("10.0.0.5", 8080);
        // client, edge, ingress — strip trailing trusted → client
        let hdrs = headers(&[("x-forwarded-for", "198.51.100.9, 10.0.0.2, 192.168.1.3")]);
        let resolved = resolve_client_ip(peer_addr, &hdrs, &trusted);
        assert_eq!(
            resolved.client_ip,
            IpAddr::from_str("198.51.100.9").unwrap()
        );
        assert!(resolved.peer_trusted);
        assert_eq!(resolved.source, ClientIpSource::XForwardedFor);
    }

    #[test]
    fn x_real_ip_fallback_when_no_chain() {
        let trusted = parse_trusted_proxies("10.0.0.0/8").unwrap();
        let peer_addr = peer("10.0.0.5", 8080);
        let hdrs = headers(&[("x-real-ip", "198.51.100.42")]);
        let resolved = resolve_client_ip(peer_addr, &hdrs, &trusted);
        assert_eq!(
            resolved.client_ip,
            IpAddr::from_str("198.51.100.42").unwrap()
        );
        assert!(resolved.peer_trusted);
        assert_eq!(resolved.source, ClientIpSource::XRealIp);
    }

    #[test]
    fn forwarded_header_preferred() {
        let trusted = parse_trusted_proxies("10.0.0.0/8").unwrap();
        let peer_addr = peer("10.0.0.5", 8080);
        let hdrs = headers(&[
            ("forwarded", "for=198.51.100.7;proto=https, for=10.0.0.2"),
            ("x-forwarded-for", "203.0.113.1"),
            ("x-real-ip", "203.0.113.2"),
        ]);
        let resolved = resolve_client_ip(peer_addr, &hdrs, &trusted);
        assert_eq!(
            resolved.client_ip,
            IpAddr::from_str("198.51.100.7").unwrap()
        );
        assert!(resolved.peer_trusted);
        assert_eq!(resolved.source, ClientIpSource::Forwarded);
    }

    #[test]
    fn forwarded_quoted_ipv6() {
        let trusted = parse_trusted_proxies("fc00::/7").unwrap();
        let peer_addr: SocketAddr = "[fc00::1]:443".parse().unwrap();
        let hdrs = headers(&[("forwarded", r#"for="[2001:db8::9]";proto=https"#)]);
        let resolved = resolve_client_ip(peer_addr, &hdrs, &trusted);
        assert_eq!(resolved.client_ip, IpAddr::from_str("2001:db8::9").unwrap());
        assert!(resolved.peer_trusted);
        assert_eq!(resolved.source, ClientIpSource::Forwarded);
    }
}
