//! The desktop shell's library surface: `main` links it to build the window,
//! and the integration tests under `tests/` link it to drive the same request
//! path with no window at all.

pub mod db;
pub mod transport;

/// The scheme the SPA and the API are both served from.
pub const SCHEME: &str = "nigel";

/// The origin form Tauri gives a custom scheme, which differs by platform.
pub fn scheme_url() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost/")
    } else {
        format!("{SCHEME}://localhost/")
    }
}

/// The `Host` header Tauri sends for this scheme, which the router's host
/// guard must be given and nothing else.
pub fn trusted_host() -> String {
    if cfg!(windows) {
        format!("{SCHEME}.localhost")
    } else {
        "localhost".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_url_matches_the_platform_origin_form() {
        // Tauri serves a custom scheme from a different origin per platform.
        // Getting this wrong means the host guard refuses every request.
        let url = scheme_url();
        if cfg!(windows) {
            assert_eq!(url, "http://nigel.localhost/");
        } else {
            assert_eq!(url, "nigel://localhost/");
        }
        assert!(url.starts_with(SCHEME) || url.contains(SCHEME));
    }

    #[test]
    fn the_trusted_host_matches_the_host_tauri_actually_sends() {
        // Pinned to a literal rather than derived, because the integration
        // tests hand this same value to both the request and the guard: a wrong
        // value would move both sides together and leave them green while every
        // real launch answers 403.
        if cfg!(windows) {
            assert_eq!(trusted_host(), "nigel.localhost");
        } else {
            assert_eq!(trusted_host(), "localhost");
        }
    }

    #[test]
    fn the_trusted_host_is_the_authority_of_the_scheme_url() {
        // The two are one fact spelled twice; this is what keeps them in step.
        let url = scheme_url();
        let authority = url
            .split("://")
            .nth(1)
            .expect("scheme url has an authority")
            .trim_end_matches('/');
        assert_eq!(authority, trusted_host());
    }
}
