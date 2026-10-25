//! How `nigel serve` is reached, read from settings.json and the environment:
//! its one public address, and the header that carries a client's IP behind
//! a proxy on the same machine.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::{NigelError, Result};
use crate::settings::Settings;

pub const PUBLIC_URL_ENV: &str = "NIGEL_SERVER_PUBLIC_URL";
pub const CLIENT_IP_HEADER_ENV: &str = "NIGEL_CLIENT_IP_HEADER";

/// The one non-loopback origin this server answers to in shared mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    /// `host` or `host:port`, lower-cased, an IPv6 literal canonical and
    /// bracketed. The default port 443 is never written.
    authority: String,
    explicit_port: bool,
    origin: String,
}

impl PublicUrl {
    /// One `https://` origin: scheme, host, an optional port, at most a
    /// trailing `/`. Loopback names are refused, because loopback is always
    /// accepted and `Secure` cookies and HSTS must never reach it.
    pub fn parse(raw: &str) -> std::result::Result<Self, String> {
        let value = raw.trim();
        let refuse = |why: &str| {
            format!(
                "server_public_url {value:?} {why}. Use one https:// origin with no path, \
                 such as \"https://books.example.com\"."
            )
        };
        let rest = value
            .get(..8)
            .filter(|scheme| scheme.eq_ignore_ascii_case("https://"))
            .map(|_| &value[8..])
            .ok_or_else(|| refuse("is not an https:// address"))?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        if rest.is_empty() {
            return Err(refuse("has no host"));
        }
        if rest.contains(['/', '?', '#', '@', '\\'])
            || rest.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(refuse("has a path, a query or credentials in it"));
        }

        let (host, port) = split_host_port(rest).ok_or_else(|| refuse("does not parse"))?;
        let host = host.to_ascii_lowercase();
        if ends_in_a_number(&host) && host.parse::<Ipv4Addr>().is_err() {
            return Err(refuse(
                "is a numeric address that is not four dotted decimal parts",
            ));
        }
        if is_loopback(&host) {
            return Err(refuse(
                "names the loopback interface, which Nigel always accepts already",
            ));
        }
        let port = match port {
            None => None,
            Some(p) => match p
                .bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| p.parse::<u16>().ok())
                .flatten()
            {
                None | Some(0) => {
                    return Err(refuse("has a port that is not a number from 1 to 65535"))
                }
                Some(443) => None,
                Some(p) => Some(p),
            },
        };
        let host_part = match host.parse::<Ipv6Addr>() {
            Ok(ip) => format!("[{ip}]"),
            Err(_) => host,
        };
        let authority = match port {
            Some(p) => format!("{host_part}:{p}"),
            None => host_part,
        };
        Ok(Self {
            origin: format!("https://{authority}"),
            explicit_port: port.is_some(),
            authority,
        })
    }

    /// `host[:port]` as a `Host` header names it.
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// `https://host[:port]`, as a browser sends it in `Origin`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// True when a `Host` header names this address, whole.
    pub fn matches_host(&self, header: &str) -> bool {
        let header = header.trim().to_ascii_lowercase();
        header == self.authority
            || (!self.explicit_port && header == format!("{}:443", self.authority))
    }

    /// True when an `Origin` header is this origin, whole: scheme, host and port.
    pub fn matches_origin(&self, origin: &str) -> bool {
        let origin = origin.trim();
        origin.eq_ignore_ascii_case(&self.origin)
            || (!self.explicit_port && origin.eq_ignore_ascii_case(&format!("{}:443", self.origin)))
    }
}

/// `(host, port)` from an authority with no userinfo. A hostname is dot-
/// separated labels of letters, digits and hyphens; an IPv6 literal is
/// bracketed. `None` for anything else.
fn split_host_port(authority: &str) -> Option<(String, Option<&str>)> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (inner, after) = rest.split_once(']')?;
        inner.parse::<Ipv6Addr>().ok()?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?),
        };
        return Some((inner.to_string(), port));
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let valid_label = |label: &str| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    if host.split('.').all(valid_label) {
        Some((host.to_string(), port))
    } else {
        None
    }
}

/// True when a browser would read `host` as an IPv4 address: its last label
/// is decimal digits or `0x` and hex digits. A browser accepts shorthand such
/// as `127.1`, `2130706433` or `0x7f.1` for `127.0.0.1`, so only the four
/// dotted decimal parts `Ipv4Addr` parses are let through.
fn ends_in_a_number(host: &str) -> bool {
    let last = host.rsplit('.').next().unwrap_or(host);
    let hex = last
        .strip_prefix("0x")
        .is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_hexdigit()));
    hex || (!last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()))
}

/// True when `host` reaches this machine: `localhost` and any name under it,
/// a loopback or unspecified address, and their IPv4-mapped IPv6 forms.
fn is_loopback(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    let local_v4 = |ip: Ipv4Addr| ip.is_loopback() || ip.is_unspecified();
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => local_v4(ip),
        Ok(IpAddr::V6(ip)) => {
            ip.is_loopback() || ip.is_unspecified() || ip.to_ipv4_mapped().is_some_and(local_v4)
        }
        Err(_) => false,
    }
}

/// Which header carries the client's address when a proxy on this machine
/// forwards to Nigel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIpHeader {
    CfConnectingIp,
    XForwardedFor,
}

impl ClientIpHeader {
    pub fn parse(raw: &str) -> std::result::Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cf-connecting-ip" => Ok(Self::CfConnectingIp),
            "x-forwarded-for" => Ok(Self::XForwardedFor),
            _ => Err(format!(
                "client_ip_header {:?} is not one Nigel reads. Use \"CF-Connecting-IP\" or \
                 \"X-Forwarded-For\".",
                raw.trim()
            )),
        }
    }

    /// The header's name as `http` spells it.
    pub fn header_name(self) -> &'static str {
        match self {
            Self::CfConnectingIp => "cf-connecting-ip",
            Self::XForwardedFor => "x-forwarded-for",
        }
    }
}

/// What `nigel serve` reads about how it is reached. `Default` is a server
/// with neither key set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerConfig {
    pub public_url: Option<PublicUrl>,
    pub client_ip_header: Option<ClientIpHeader>,
}

impl ServerConfig {
    pub fn from_settings(settings: &Settings) -> Result<Self> {
        Self::from_settings_with(settings, |name| std::env::var(name).ok())
    }

    /// The environment wins over settings.json, each trimmed, a blank value
    /// counting as unset. An invalid value is an error naming it.
    pub fn from_settings_with(
        settings: &Settings,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self> {
        let pick = |name: &str, file: &Option<String>| {
            let trimmed =
                |v: Option<String>| v.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
            trimmed(env(name)).or_else(|| trimmed(file.clone()))
        };
        let public_url = pick(PUBLIC_URL_ENV, &settings.server_public_url)
            .map(|v| PublicUrl::parse(&v))
            .transpose()
            .map_err(NigelError::Other)?;
        let client_ip_header = pick(CLIENT_IP_HEADER_ENV, &settings.client_ip_header)
            .map(|v| ClientIpHeader::parse(&v))
            .transpose()
            .map_err(NigelError::Other)?;
        Ok(Self {
            public_url,
            client_ip_header,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(raw: &str) -> PublicUrl {
        PublicUrl::parse(raw).unwrap_or_else(|e| panic!("{raw}: {e}"))
    }

    #[test]
    fn an_https_origin_parses_to_its_host_and_origin() {
        let u = url("https://books.example.com");
        assert_eq!(u.authority(), "books.example.com");
        assert_eq!(u.origin(), "https://books.example.com");
        let u = url("https://books.example.com:8443");
        assert_eq!(u.authority(), "books.example.com:8443");
        assert_eq!(u.origin(), "https://books.example.com:8443");
        let u = url("https://[2001:db8::1]");
        assert_eq!(u.authority(), "[2001:db8::1]");
    }

    #[test]
    fn loopback_and_loose_spellings() {
        for loopback in [
            "https://localhost",
            "https://127.0.0.1",
            "https://127.1.2.3:8443",
            "https://[::1]",
            "https://LOCALHOST/",
        ] {
            let err = PublicUrl::parse(loopback).unwrap_err();
            assert!(err.contains("loopback"), "{loopback}: {err}");
        }
        for (raw, origin) in [
            ("https://BOOKS.example.com/", "https://books.example.com"),
            ("HTTPS://books.example.com:443", "https://books.example.com"),
            ("  https://books.example.com  ", "https://books.example.com"),
        ] {
            assert_eq!(url(raw).origin(), origin, "{raw}");
        }
    }

    #[test]
    fn anything_but_one_https_origin_is_refused_by_value() {
        for raw in [
            "http://books.example.com",
            "books.example.com",
            "https://",
            "https://books.example.com/nigel",
            "https://books.example.com?x=1",
            "https://books.example.com#top",
            "https://pat@books.example.com",
            "https://books.example.com:0",
            "https://books.example.com:99999",
            "https://books.example.com:abc",
            "https://bad_host.example.com",
            "https://books..example.com",
            "https://.example.com",
            "https://books example.com",
            "https://[not-v6]",
        ] {
            let err = PublicUrl::parse(raw).unwrap_err();
            assert!(err.contains("server_public_url"), "{raw}: {err}");
            assert!(err.contains(raw.trim()), "{raw}: {err}");
        }
    }

    #[test]
    fn the_host_header_and_origin_match_whole() {
        let u = url("https://books.example.com");
        assert!(u.matches_host("books.example.com"));
        assert!(u.matches_host("Books.Example.com"));
        assert!(u.matches_host("books.example.com:443"));
        assert!(!u.matches_host("books.example.com:8443"));
        assert!(!u.matches_host("evil.books.example.com"));
        assert!(!u.matches_host("books.example.com.evil.com"));
        assert!(u.matches_origin("https://books.example.com"));
        assert!(u.matches_origin("https://books.example.com:443"));
        assert!(!u.matches_origin("http://books.example.com"));
        assert!(!u.matches_origin("https://books.example.com:8443"));
        assert!(!u.matches_origin("https://books.example.com.evil.com"));
        assert!(!u.matches_origin("null"));

        let p = url("https://books.example.com:8443");
        assert!(p.matches_host("books.example.com:8443"));
        assert!(!p.matches_host("books.example.com"));
        assert!(p.matches_origin("https://books.example.com:8443"));
        assert!(!p.matches_origin("https://books.example.com"));
    }

    #[test]
    fn the_client_ip_header_is_one_of_two_names_in_any_case() {
        assert_eq!(
            ClientIpHeader::parse("CF-Connecting-IP").unwrap(),
            ClientIpHeader::CfConnectingIp
        );
        assert_eq!(
            ClientIpHeader::parse(" x-forwarded-for ").unwrap(),
            ClientIpHeader::XForwardedFor
        );
        let err = ClientIpHeader::parse("X-Real-IP").unwrap_err();
        assert!(
            err.contains("X-Real-IP") && err.contains("client_ip_header"),
            "{err}"
        );
    }

    #[test]
    fn the_environment_wins_and_blank_values_are_unset() {
        let file = Settings {
            server_public_url: Some("https://file.example.com".into()),
            client_ip_header: Some("X-Forwarded-For".into()),
            ..Settings::default()
        };
        let none = ServerConfig::from_settings_with(&file, |_| None).unwrap();
        assert_eq!(
            none.public_url.unwrap().origin(),
            "https://file.example.com"
        );
        assert_eq!(none.client_ip_header, Some(ClientIpHeader::XForwardedFor));

        let env = ServerConfig::from_settings_with(&file, |name| match name {
            PUBLIC_URL_ENV => Some("https://env.example.com".into()),
            CLIENT_IP_HEADER_ENV => Some("   ".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(env.public_url.unwrap().origin(), "https://env.example.com");
        assert_eq!(env.client_ip_header, Some(ClientIpHeader::XForwardedFor));

        let bad = ServerConfig::from_settings_with(&file, |name| {
            (name == PUBLIC_URL_ENV).then(|| "http://env.example.com".to_string())
        })
        .unwrap_err()
        .to_string();
        assert!(bad.contains("http://env.example.com"), "{bad}");

        assert_eq!(
            ServerConfig::from_settings_with(&Settings::default(), |_| None).unwrap(),
            ServerConfig::default()
        );
    }

    #[test]
    fn every_spelling_a_browser_reads_as_this_machine_is_loopback() {
        for raw in [
            "https://127.1",
            "https://2130706433",
            "https://0x7f.1",
            "https://0x7F000001",
            "https://0177.0.0.1",
            "https://books.localhost",
            "https://[::ffff:127.0.0.1]",
            "https://0.0.0.0",
            "https://[::]",
        ] {
            let err = PublicUrl::parse(raw).unwrap_err();
            assert!(err.contains(raw), "{raw}: {err}");
        }
        assert_eq!(url("https://203.0.113.9").origin(), "https://203.0.113.9");
        assert_eq!(
            url("https://books.example.com:00443").origin(),
            "https://books.example.com"
        );
    }

    #[test]
    fn a_port_is_digits_and_an_ipv6_literal_is_canonical() {
        let err = PublicUrl::parse("https://books.example.com:+8443").unwrap_err();
        assert!(err.contains("port"), "{err}");
        let u = url("https://[2001:DB8:0::1]");
        assert_eq!(u.authority(), "[2001:db8::1]");
        assert!(u.matches_host("[2001:db8::1]"));
    }
}
