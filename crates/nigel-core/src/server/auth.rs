//! Session token, cookie handling, and the Host/Origin guard.
//!
//! Localhost is not a trust boundary on its own: any web page the user visits
//! can issue requests to `127.0.0.1`, and a hostile DNS record can point a
//! public name at the loopback address. The guards here are the two cheap
//! defenses against that — an exact-match Host/Origin check to defeat DNS
//! rebinding, and a per-run session token that a drive-by page cannot read.

use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use rand::RngCore;
use serde::Deserialize;
use subtle::ConstantTimeEq;

use super::error::ApiError;
use super::state::AppState;

pub const SESSION_COOKIE: &str = "nigel_session";

const TOKEN_BYTES: usize = 32;

/// The hosts that may address a router.
///
/// A parameter rather than a constant because the desktop client is served from
/// a custom URI scheme whose origin is neither loopback nor the same string on
/// every platform. Matching stays exact — `127.0.0.1.evil.com` and
/// `localhost.evil.com` resolve wherever their owner points them.
#[derive(Clone, Debug)]
pub struct TrustedOrigins {
    hosts: Vec<String>,
    /// Whole origins trusted verbatim, for schemes that are not http(s).
    ///
    /// A desktop shell serves the app from its own URI scheme, whose origin
    /// (`nigel://localhost`) has no host-and-port shape to compare — so it is
    /// matched as a complete string or not at all.
    origins: Vec<String>,
}

impl TrustedOrigins {
    /// What `nigel serve` allows: the loopback interface, by any of its names.
    pub fn loopback() -> Self {
        Self {
            hosts: ["localhost", "127.0.0.1", "::1"]
                .iter()
                .map(|h| (*h).to_string())
                .collect(),
            origins: Vec::new(),
        }
    }

    /// Exactly these hosts and nothing else — not loopback unless it is listed.
    pub fn exactly(hosts: Vec<String>) -> Self {
        Self {
            hosts,
            origins: Vec::new(),
        }
    }

    /// Also trust these whole origins, for a scheme that is not http(s).
    ///
    /// Each is compared in full, so listing `nigel://localhost` trusts that and
    /// nothing else — not `nigel://elsewhere`, and not a host of `localhost`
    /// under any other scheme.
    pub fn and_origins(mut self, origins: Vec<String>) -> Self {
        self.origins = origins;
        self
    }

    /// True when a `Host` header names a trusted host, with any port.
    pub fn host_is_trusted(&self, host: &str) -> bool {
        let host = host.trim();
        if host.is_empty() {
            return false;
        }

        let bare = if let Some(rest) = host.strip_prefix('[') {
            let Some((inner, after)) = rest.split_once(']') else {
                return false;
            };
            match after.strip_prefix(':') {
                Some(port) if is_port(port) => {}
                None if after.is_empty() => {}
                _ => return false,
            }
            inner
        } else {
            match host.split_once(':') {
                Some((h, port)) if is_port(port) => h,
                Some(_) => return false,
                None => host,
            }
        };

        self.hosts.iter().any(|h| bare.eq_ignore_ascii_case(h))
    }

    /// True when an `Origin` header names an http(s) trusted origin. A literal
    /// `null` origin (sandboxed iframes, `file://` documents) is never trusted.
    pub fn origin_is_trusted(&self, origin: &str) -> bool {
        let origin = origin.trim();

        // A whole-origin match comes first: a custom scheme has no
        // host-and-port shape for the http(s) path below to read.
        if self
            .origins
            .iter()
            .any(|trusted| trusted.eq_ignore_ascii_case(origin))
        {
            return true;
        }

        let Some(rest) = strip_scheme(origin) else {
            return false;
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let authority = match authority.rsplit_once('@') {
            Some((_, host)) => host,
            None => authority,
        };
        self.host_is_trusted(authority)
    }
}

/// A fresh session token for this server run. `ThreadRng` is a CSPRNG.
pub fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Constant-time token comparison. Empty tokens never match, so a server that
/// somehow started without a token cannot be opened with an empty cookie.
pub fn tokens_match(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

fn is_port(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// True when a `Host` header names the loopback interface, with any port.
pub fn host_is_local(host: &str) -> bool {
    TrustedOrigins::loopback().host_is_trusted(host)
}

fn strip_scheme(origin: &str) -> Option<&str> {
    ["http://", "https://"].into_iter().find_map(|scheme| {
        let head = origin.get(..scheme.len())?;
        head.eq_ignore_ascii_case(scheme)
            .then(|| &origin[scheme.len()..])
    })
}

/// True when an `Origin` header names an http(s) loopback origin. A literal
/// `null` origin (sandboxed iframes, `file://` documents) is never local.
pub fn origin_is_local(origin: &str) -> bool {
    TrustedOrigins::loopback().origin_is_trusted(origin)
}

/// Look up one cookie in a `Cookie` header value.
pub fn cookie_value<'a>(cookies: &'a str, name: &str) -> Option<&'a str> {
    cookies.split(';').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key.trim() == name {
            Some(value.trim())
        } else {
            None
        }
    })
}

/// Outermost layer: reject anything not addressed to a trusted host.
pub async fn host_guard(
    State(trusted): State<TrustedOrigins>,
    req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();

    let host_ok = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| trusted.host_is_trusted(host));
    if !host_ok {
        return ApiError::forbidden().into_response();
    }

    if let Some(origin) = headers.get(header::ORIGIN) {
        let origin_ok = origin
            .to_str()
            .is_ok_and(|origin| trusted.origin_is_trusted(origin));
        if !origin_ok {
            return ApiError::forbidden().into_response();
        }
    }

    next.run(req).await
}

/// Layered over `/api` only — `/auth` and the static assets carry no data and
/// are reachable without a session.
pub async fn session_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let authorized = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookies| cookie_value(cookies, SESSION_COOKIE))
        .any(|token| tokens_match(token, &state.session_token));

    if authorized {
        next.run(req).await
    } else {
        ApiError::unauthorized().into_response()
    }
}

#[derive(Deserialize)]
pub struct AuthQuery {
    /// Optional so a bare `/auth` is an unauthorized session rather than a
    /// deserialization rejection.
    token: Option<String>,
}

/// `GET /auth?token=…` — trades the one-shot URL token for a session cookie.
pub async fn auth_redirect(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
) -> Response {
    let token = query.token.unwrap_or_default();
    if !tokens_match(&token, &state.session_token) {
        return ApiError::unauthorized().into_response();
    }

    let cookie = format!("{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/");
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "/")
        .header(header::SET_COOKIE, cookie)
        .body(Body::empty())
        .unwrap_or_else(|e| ApiError::internal(e).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_hosts() {
        for host in [
            "localhost",
            "localhost:5731",
            "LocalHost:5173",
            "127.0.0.1",
            "127.0.0.1:5173",
            "[::1]",
            "[::1]:5731",
            " localhost:5731 ",
        ] {
            assert!(host_is_local(host), "expected {host} to be local");
        }
    }

    #[test]
    fn rejects_non_loopback_hosts() {
        for host in [
            "evil.com",
            "evil.com:5731",
            "127.0.0.1.evil.com",
            "localhost.evil.com",
            "0.0.0.0:5731",
            "192.168.1.5:5731",
            "[::]",
            "",
            "127.0.0.1:5731:80",
            "127.0.0.1:abc",
            "::1",
            "[::1",
        ] {
            assert!(!host_is_local(host), "expected {host} to be rejected");
        }
    }

    #[test]
    fn accepts_loopback_origins() {
        for origin in [
            "http://localhost:5173",
            "http://127.0.0.1:5731",
            "https://localhost",
            "HTTP://localhost:5173",
            "http://[::1]:5731",
            "http://localhost:5173/",
        ] {
            assert!(origin_is_local(origin), "expected {origin} to be local");
        }
    }

    #[test]
    fn rejects_non_loopback_origins() {
        for origin in [
            "http://evil.com",
            "null",
            "file://",
            "http://127.0.0.1.evil.com",
            "https://evil.com:5731",
            "",
            "localhost:5731",
        ] {
            assert!(!origin_is_local(origin), "expected {origin} to be rejected");
        }
    }

    #[test]
    fn tokens_match_only_when_identical() {
        assert!(tokens_match("abc123", "abc123"));
        assert!(!tokens_match("abc123", "abc124"));
        assert!(!tokens_match("abc123", "abc1234"));
        assert!(!tokens_match("", ""));
        assert!(!tokens_match("abc", ""));
    }

    #[test]
    fn parses_cookie_values() {
        assert_eq!(
            cookie_value("nigel_session=abc", "nigel_session"),
            Some("abc")
        );
        assert_eq!(
            cookie_value("theme=dark; nigel_session=abc; other=1", "nigel_session"),
            Some("abc")
        );
        assert_eq!(cookie_value("theme=dark", "nigel_session"), None);
        assert_eq!(cookie_value("xnigel_session=abc", "nigel_session"), None);
        assert_eq!(cookie_value("", "nigel_session"), None);
        assert_eq!(cookie_value("nigel_session=", "nigel_session"), Some(""));
    }

    #[test]
    fn tokens_are_random_hex() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), TOKEN_BYTES * 2);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn loopback_trust_is_what_serve_has_always_allowed() {
        let trusted = TrustedOrigins::loopback();
        assert!(trusted.host_is_trusted("localhost"));
        assert!(trusted.host_is_trusted("127.0.0.1:5731"));
        assert!(trusted.host_is_trusted("[::1]:5731"));
        assert!(!trusted.host_is_trusted("127.0.0.1.evil.com"));
        assert!(!trusted.host_is_trusted("localhost.evil.com"));
        assert!(!trusted.host_is_trusted("nigel.localhost"));
    }

    #[test]
    fn origin_userinfo_does_not_smuggle_in_a_trusted_host() {
        // `rsplit_once('@')` takes the half after the last `@`, which is the
        // host; taking the half before would trust whatever the attacker put
        // in the userinfo slot.
        assert!(!origin_is_local("http://localhost@evil.com"));
    }

    #[test]
    fn a_non_numeric_bracketed_port_is_rejected() {
        assert!(!host_is_local("[::1]:abc"));
    }

    #[test]
    fn a_whole_origin_is_trusted_only_when_it_is_listed() {
        // A desktop shell serves the app from its own scheme, whose origin has
        // no host-and-port shape — so it is matched whole or not at all.
        let trusted = TrustedOrigins::exactly(vec!["localhost".to_string()])
            .and_origins(vec!["nigel://localhost".to_string()]);

        assert!(trusted.origin_is_trusted("nigel://localhost"));
        assert!(trusted.origin_is_trusted("NIGEL://LOCALHOST"));

        // Neither another authority under the same scheme...
        assert!(!trusted.origin_is_trusted("nigel://elsewhere"));
        // ...nor the same authority under another scheme.
        assert!(!trusted.origin_is_trusted("evil://localhost"));
        // A literal `null` origin stays refused, listed origins or not.
        assert!(!trusted.origin_is_trusted("null"));
    }

    #[test]
    fn listing_a_whole_origin_does_not_widen_the_http_path() {
        // Trusting nigel://localhost must not make http://localhost trusted by
        // accident, nor the reverse.
        let trusted =
            TrustedOrigins::exactly(vec![]).and_origins(vec!["nigel://localhost".to_string()]);

        assert!(trusted.origin_is_trusted("nigel://localhost"));
        assert!(!trusted.origin_is_trusted("http://localhost"));
        assert!(!trusted.host_is_trusted("localhost"));
    }

    #[test]
    fn a_configured_origin_is_trusted_and_loopback_then_is_not() {
        let trusted = TrustedOrigins::exactly(vec!["nigel.localhost".to_string()]);
        assert!(trusted.host_is_trusted("nigel.localhost"));
        assert!(trusted.origin_is_trusted("http://nigel.localhost"));
        // A desktop router has no reason to answer the loopback interface, and
        // trusting it anyway would hand the guard back the hole it exists to close.
        assert!(!trusted.host_is_trusted("127.0.0.1:5731"));
        assert!(!trusted.host_is_trusted("localhost"));
        assert!(!trusted.host_is_trusted("evil.nigel.localhost"));
    }
}
