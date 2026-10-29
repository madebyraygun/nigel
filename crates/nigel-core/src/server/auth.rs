//! Session token, cookie handling, and the Host/Origin guard.
//!
//! Localhost is not a trust boundary on its own: any web page the user visits
//! can issue requests to `127.0.0.1`, and a hostile DNS record can point a
//! public name at the loopback address. The guards here are the two cheap
//! defenses against that — an exact-match Host/Origin check to defeat DNS
//! rebinding, and a per-run session token that a drive-by page cannot read.

use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use rand::RngCore;
use serde::Deserialize;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use super::error::ApiError;
use super::mode::Mode;
use super::principal::{Principal, SessionUser};
use super::state::AppState;
use crate::identity::sessions::{self, ActiveSession, SessionKind};

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

/// The request's `Host`, when it carries exactly one. A request with two
/// names no single host, so neither guard trusts either of them.
fn sole_host(headers: &axum::http::HeaderMap) -> Option<&str> {
    let mut hosts = headers.get_all(header::HOST).iter();
    match (hosts.next(), hosts.next()) {
        (Some(host), None) => host.to_str().ok(),
        _ => None,
    }
}

/// Outermost layer: reject anything not addressed to a trusted host.
pub async fn host_guard(
    State(trusted): State<TrustedOrigins>,
    req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();

    let host_ok = sole_host(headers).is_some_and(|host| trusted.host_is_trusted(host));
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

/// True when the request's `Host` is this server's `server_public_url`: the
/// request reached it over https through a proxy, so cookies are `Secure` and
/// the answer carries HSTS. Loopback is never public.
pub(crate) fn request_is_public(state: &AppState, headers: &axum::http::HeaderMap) -> bool {
    let Some(public) = state.config.public_url.as_ref() else {
        return false;
    };
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| public.matches_host(host))
}

/// Outermost guard of `nigel serve`. The `Host` must be loopback by any of
/// its names, or the `server_public_url` host while the server is shared. An
/// `Origin`, when sent, must be exactly the `server_public_url` origin on a
/// public-host request; on a loopback-host request it may also be a loopback
/// origin on any port, and is the public origin only while shared. A
/// public-host request to a local-mode server re-derives the mode first, so a
/// first user added with the CLI takes effect without a restart.
pub async fn served_host_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let loopback = TrustedOrigins::loopback();
    let host = sole_host(req.headers()).map(str::to_string);
    let origin = req
        .headers()
        .get(header::ORIGIN)
        .map(|value| value.to_str().map(str::to_string));
    let Some(host) = host else {
        return ApiError::forbidden().into_response();
    };

    let public = state.config.public_url.clone();
    let via_public = public.as_ref().is_some_and(|p| p.matches_host(&host));
    if !via_public && !loopback.host_is_trusted(&host) {
        return ApiError::forbidden().into_response();
    }

    let mut mode = state.mode();
    if via_public && mode == super::mode::Mode::Local {
        mode = state.refresh_mode_throttled().await;
    }
    let shared = mode == super::mode::Mode::Shared;
    if via_public && !shared {
        return ApiError::forbidden().into_response();
    }

    if let Some(origin) = origin {
        let trusted = origin.is_ok_and(|origin| {
            let public_origin =
                shared && public.as_ref().is_some_and(|p| p.matches_origin(&origin));
            public_origin || (!via_public && loopback.origin_is_trusted(&origin))
        });
        if !trusted {
            return ApiError::forbidden().into_response();
        }
    }

    next.run(req).await
}

/// The one `/api` path that needs no session, as seen inside the nest.
pub const LOGIN_PATH: &str = "/auth/login";

/// What a user who must change a temporary password may still call.
const MUST_CHANGE_ALLOWED: [(Method, &str); 5] = [
    (Method::GET, "/auth/me"),
    (Method::GET, "/status"),
    (Method::GET, "/ping"),
    (Method::POST, "/account/password"),
    (Method::POST, "/auth/logout"),
];

/// `nigel_session=<token>; HttpOnly; SameSite=Strict; Path=/; Max-Age=<n>`,
/// with `; Secure` for a request to the public host.
pub fn session_cookie(token: &str, max_age_secs: i64, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age_secs}{}",
        if secure { "; Secure" } else { "" }
    )
}

/// The cookie that ends a browser session.
pub fn expired_cookie(secure: bool) -> String {
    session_cookie("", 0, secure)
}

fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// The credential a request presents: `Authorization: Bearer` when that
/// header is present (a malformed one is `None`, never a fall-back to the
/// cookie), otherwise the `nigel_session` cookie.
fn presented_credential(headers: &HeaderMap) -> Option<(Zeroizing<String>, SessionKind)> {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        let value = value.to_str().ok()?;
        let scheme = value.get(..7)?;
        if !scheme.eq_ignore_ascii_case("bearer ") {
            return None;
        }
        let token = value[7..].trim();
        return Some((Zeroizing::new(token.to_string()), SessionKind::Bearer));
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|cookies| cookie_value(cookies, SESSION_COOKIE))
        .map(|token| (Zeroizing::new(token.to_string()), SessionKind::Cookie))
}

fn seconds_until(expires_at: &str, now: chrono::DateTime<chrono::Utc>) -> Option<i64> {
    let at = chrono::DateTime::parse_from_rfc3339(expires_at)
        .ok()?
        .with_timezone(&chrono::Utc);
    Some((at - now).num_seconds().max(0))
}

/// Layered over `/api` only — `/auth` and the static assets carry no data.
///
/// Local mode: the per-run token cookie, and the request runs as the local
/// owner. Shared mode: a session looked up by the hash of its token, joined
/// to an enabled user, unexpired; the user's role is read on every request.
/// `POST /api/auth/login` passes without one.
pub async fn session_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if req.uri().path() == LOGIN_PATH {
        return next.run(req).await;
    }
    match state.mode() {
        Mode::Local => local_session(state, req, next).await,
        Mode::Shared => shared_session(state, req, next).await,
    }
}

/// A token-cookie request re-derives the mode (throttled) before it is let
/// through, so a first user added from another process ends the token
/// session on loopback too, not only once the public host is used.
async fn local_session(state: AppState, mut req: Request, next: Next) -> Response {
    let Some(expected) = state.local_token() else {
        return ApiError::unauthorized_shared().into_response();
    };
    let authorized = req
        .headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookies| cookie_value(cookies, SESSION_COOKIE))
        .any(|token| tokens_match(token, expected.expose()));
    if !authorized {
        return ApiError::unauthorized().into_response();
    }
    if state.refresh_mode_throttled().await == Mode::Shared {
        return ApiError::unauthorized_shared().into_response();
    }
    req.extensions_mut().insert(Principal::LocalOwner);
    next.run(req).await
}

async fn shared_session(state: AppState, mut req: Request, next: Next) -> Response {
    let Some((token, via)) = presented_credential(req.headers()) else {
        return ApiError::unauthorized_shared().into_response();
    };

    let now = state.now();
    let lookup = token.clone();
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let found = super::routes::spawn_blocking_holding(
        gate,
        move || -> crate::error::Result<Option<(ActiveSession, Option<String>)>> {
            let conn = crate::db::get_connection(&db_path)?;
            let Some(session) = sessions::find_session(&conn, &lookup, now)? else {
                return Ok(None);
            };
            let renewed = sessions::renew_session(&conn, &session, now)?;
            Ok(Some((session, renewed)))
        },
    )
    .await;
    let (session, renewed) = match found {
        Ok((_gate, Ok(Some(found)))) => found,
        Ok((_gate, Ok(None))) => return ApiError::unauthorized_shared().into_response(),
        Ok((_gate, Err(e))) => return ApiError::from(e).into_response(),
        Err(e) => return e.into_response(),
    };
    if session.kind != via {
        return ApiError::unauthorized_shared().into_response();
    }

    let method = req.method().clone();
    if via == SessionKind::Cookie
        && !is_safe_method(&method)
        && !req.headers().contains_key(header::ORIGIN)
    {
        return ApiError::origin_required().into_response();
    }
    if session.must_change_password {
        let path = req.uri().path();
        let allowed = MUST_CHANGE_ALLOWED
            .iter()
            .any(|(m, p)| *m == method && *p == path);
        if !allowed {
            return ApiError::password_change_required().into_response();
        }
    }

    let secure = request_is_public(&state, req.headers());
    req.extensions_mut()
        .insert(Principal::User(SessionUser::from(&session)));
    let mut response = next.run(req).await;

    if let (SessionKind::Cookie, Some(expires_at)) = (session.kind, renewed) {
        if !response.headers().contains_key(header::SET_COOKIE) {
            let cookie = seconds_until(&expires_at, now).and_then(|max_age| {
                HeaderValue::from_str(&session_cookie(&token, max_age, secure)).ok()
            });
            if let Some(cookie) = cookie {
                response.headers_mut().append(header::SET_COOKIE, cookie);
            }
        }
    }
    response
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
    let Some(expected) = state.local_token() else {
        return ApiError::unauthorized_shared().into_response();
    };
    let token = query.token.unwrap_or_default();
    if !tokens_match(&token, expected.expose()) {
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
