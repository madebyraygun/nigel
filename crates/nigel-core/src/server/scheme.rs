//! The custom URI scheme a desktop shell serves the SPA and the API from.
//!
//! Every shell — the Tauri one and the Swift one over `nigel-ffi` — navigates
//! to [`scheme_url`] and hands [`trusted_origins`] to the router's guard, so
//! the origin is decided once and cannot drift between them. The values are
//! pinned to literals by the tests below rather than derived from each other:
//! a wrong value would move the URL and the guard together and stay green
//! while every real launch answered 403.
//!
//! [`answer`] is the request path both shells share: each converts its
//! platform's request into an `http::Request`, and this hands it to the router.

use axum::body::Body;
use axum::http::{header, HeaderValue, Request, Response};
use axum::Router;
use tower::ServiceExt;

use super::auth::TrustedOrigins;

/// No response the router builds is larger than a PDF export.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

/// Answer one scheme request from the router.
///
/// The router is a `tower::Service`, so serving it needs no listener and no
/// port — which is the point: the page and its API are the same origin, so
/// there is nothing for another process on the machine to connect to. The
/// body is collected whole, because neither shell's scheme handler streams.
pub async fn answer(router: Router, request: Request<Body>) -> Response<Vec<u8>> {
    let (mut parts, body) = request.into_parts();
    carry_the_authority_as_host(&mut parts);
    let request = Request::from_parts(parts, body);

    let response = match router.oneshot(request).await {
        Ok(response) => response,
        Err(never) => match never {},
    };

    let (parts, body) = response.into_parts();
    match axum::body::to_bytes(body, MAX_RESPONSE_BYTES).await {
        Ok(collected) => Response::from_parts(parts, collected.to_vec()),
        Err(e) => Response::builder()
            .status(500)
            .body(format!("response body: {e}").into_bytes())
            .expect("build error response"),
    }
}

/// Give the request the `Host` header the router's guard reads.
///
/// A custom scheme is not HTTP, so the webview sends no `Host` — WebKitGTK and
/// WKWebView have nothing to put there, and the guard refuses a request that
/// carries none. The authority is still in the URI (`nigel://localhost/`), so
/// it moves to the header where the guard can see it. This narrows nothing: a
/// request for another authority arrives with that authority as its `Host` and
/// is refused, exactly as it would be over HTTP. Windows needs none of this —
/// its `http://nigel.localhost/` form is a real HTTP URL and carries a `Host`
/// already, which is left alone.
fn carry_the_authority_as_host(parts: &mut axum::http::request::Parts) {
    if parts.headers.contains_key(header::HOST) {
        return;
    }
    let Some(authority) = parts.uri.authority().map(|a| a.as_str().to_string()) else {
        return;
    };
    if let Ok(value) = HeaderValue::from_str(&authority) {
        parts.headers.insert(header::HOST, value);
    }
}

/// The scheme the SPA and the API are both served from.
pub const SCHEME: &str = "nigel";

/// The URL a shell's webview navigates to.
///
/// Tauri gives a custom scheme a different origin form on Windows, where
/// WebView2 only serves it as a real HTTP URL; WKWebView and WebKitGTK take
/// the scheme as written.
pub fn scheme_url() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost/")
    } else {
        format!("{SCHEME}://localhost/")
    }
}

/// The origin the webview stamps on requests the page makes.
///
/// A custom scheme's origin is the whole string, not a host and port, so the
/// guard is given it verbatim.
pub fn scheme_origin() -> String {
    scheme_url().trim_end_matches('/').to_string()
}

/// The `Host` a scheme request carries: the authority of [`scheme_url`].
pub fn trusted_host() -> String {
    if cfg!(windows) {
        format!("{SCHEME}.localhost")
    } else {
        "localhost".to_string()
    }
}

/// What the router's guard trusts: this platform's host, and the scheme's own
/// origin.
///
/// One function so the shells and their tests cannot drift — a test that built
/// its own would pass while the running app refused every request.
pub fn trusted_origins() -> TrustedOrigins {
    TrustedOrigins::exactly(vec![trusted_host()]).and_origins(vec![scheme_origin()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_url_matches_the_platform_origin_form() {
        let url = scheme_url();
        if cfg!(windows) {
            assert_eq!(url, "http://nigel.localhost/");
        } else {
            assert_eq!(url, "nigel://localhost/");
        }
    }

    #[test]
    fn the_trusted_host_matches_the_host_the_webview_actually_sends() {
        if cfg!(windows) {
            assert_eq!(trusted_host(), "nigel.localhost");
        } else {
            assert_eq!(trusted_host(), "localhost");
        }
    }

    #[test]
    fn the_trusted_host_is_the_authority_of_the_scheme_url() {
        let url = scheme_url();
        let authority = url
            .split("://")
            .nth(1)
            .expect("scheme url has an authority")
            .trim_end_matches('/');
        assert_eq!(authority, trusted_host());
    }

    #[test]
    fn the_guard_trusts_the_scheme_host_and_origin_and_nothing_else() {
        let trusted = trusted_origins();
        assert!(trusted.host_is_trusted(&trusted_host()));
        assert!(!trusted.host_is_trusted("evil.example"));
        assert!(!trusted.host_is_trusted("127.0.0.1"));
        assert!(trusted.origin_is_trusted(&scheme_origin()));
        assert!(!trusted.origin_is_trusted("nigel://elsewhere"));
    }

    fn echo_host() -> Router {
        Router::new().route(
            "/host",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                headers
                    .get(header::HOST)
                    .map(|h| h.to_str().unwrap().to_string())
                    .unwrap_or_default()
            }),
        )
    }

    #[tokio::test]
    async fn a_request_without_a_host_carries_its_authority_as_one() {
        let request = Request::get("nigel://localhost/host")
            .body(Body::empty())
            .unwrap();

        let response = answer(echo_host(), request).await;

        assert_eq!(response.status(), 200);
        assert_eq!(response.body(), b"localhost");
    }

    #[tokio::test]
    async fn a_host_the_request_already_carries_is_left_alone() {
        let request = Request::get("http://nigel.localhost/host")
            .header(header::HOST, "nigel.localhost")
            .body(Body::empty())
            .unwrap();

        let response = answer(echo_host(), request).await;

        assert_eq!(response.body(), b"nigel.localhost");
    }
}
