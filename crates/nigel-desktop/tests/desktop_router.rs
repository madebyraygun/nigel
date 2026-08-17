//! Drives `answer()` — the same adapter `main` wires into Tauri — over a real
//! temporary database, with no window and no port.
//!
//! This is a proxy for the manual "boot the window" check in the task brief,
//! not a replacement for it: it proves the router answers correctly over this
//! transport, but nothing here renders a webview or exercises the real Tauri
//! IPC plumbing.

use nigel_core::server::auth::TrustedOrigins;
use nigel_core::server::state::AppState;
use nigel_core::server::{build_desktop_router, testutil};

fn scheme_request(method: &str, path: &str, host: &str) -> tauri::http::Request<Vec<u8>> {
    tauri::http::Request::builder()
        .method(method)
        .uri(format!(
            "{}{}",
            nigel_desktop::scheme_url(),
            path.trim_start_matches('/')
        ))
        .header(tauri::http::header::HOST, host)
        .body(Vec::new())
        .expect("build scheme request")
}

fn router_over(db_path: &std::path::Path) -> axum::Router {
    let token = nigel_core::server::auth::generate_token();
    let state = AppState::new(db_path.to_path_buf(), token);
    build_desktop_router(
        state,
        TrustedOrigins::exactly(vec![nigel_desktop::trusted_host()]),
    )
}

#[tokio::test]
async fn get_root_answers_the_spa_shell() {
    let (_dir, db_path) = testutil::temp_db();
    let router = router_over(&db_path);

    let request = scheme_request("GET", "/", &nigel_desktop::trusted_host());
    let response = nigel_desktop::transport::answer(router, request).await;

    assert_eq!(response.status(), 200);
    let content_type = response
        .headers()
        .get(tauri::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with("text/html"),
        "expected text/html, got {content_type}"
    );
    let body = String::from_utf8_lossy(response.body()).to_lowercase();
    assert!(
        body.contains("<!doctype html") && body.contains("<title>nigel</title>"),
        "response body did not look like the SPA shell: {body}"
    );
}

#[tokio::test]
async fn unlock_is_reachable_over_the_scheme() {
    let (_dir, db_path) = testutil::temp_db();
    let router = router_over(&db_path);

    let request = tauri::http::Request::builder()
        .method("POST")
        .uri(format!("{}api/unlock", nigel_desktop::scheme_url()))
        .header(tauri::http::header::HOST, nigel_desktop::trusted_host())
        .header(tauri::http::header::CONTENT_TYPE, "application/json")
        .body(br#"{"password":"whatever"}"#.to_vec())
        .expect("build unlock request");

    let response = nigel_desktop::transport::answer(router, request).await;

    // temp_db() is unencrypted: the route reads the body, decides no password
    // is needed, and refuses with a 400 rather than a locked/unlocked answer.
    assert_eq!(
        response.status(),
        400,
        "unexpected status for /api/unlock over an unencrypted database: body {}",
        String::from_utf8_lossy(response.body())
    );
}

#[tokio::test]
async fn a_request_with_the_wrong_host_is_refused() {
    let (_dir, db_path) = testutil::temp_db();
    let router = router_over(&db_path);

    let request = scheme_request("GET", "/", "evil.example");
    let response = nigel_desktop::transport::answer(router, request).await;

    assert_eq!(response.status(), 403);
}

/// The shape the webview actually sends: a custom scheme is not HTTP, so there
/// is no `Host` header on the request at all.
fn scheme_request_without_host(path: &str) -> tauri::http::Request<Vec<u8>> {
    tauri::http::Request::builder()
        .method("GET")
        .uri(format!(
            "{}{}",
            nigel_desktop::scheme_url(),
            path.trim_start_matches('/')
        ))
        .body(Vec::new())
        .expect("build scheme request")
}

#[tokio::test]
async fn a_request_carrying_no_host_header_is_answered() {
    // WebKitGTK and WKWebView send no `Host` for a custom scheme, and the
    // router's guard refuses a request without one. Every screen 403s if the
    // transport does not carry the URI's authority into the header.
    let (_dir, db_path) = testutil::temp_db();
    let router = router_over(&db_path);

    let response = nigel_desktop::transport::answer(router, scheme_request_without_host("/")).await;

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn a_request_for_another_authority_is_still_refused() {
    // Carrying the authority into the header must not blunt the guard: an
    // authority we do not trust arrives as an untrusted `Host` and is refused.
    let (_dir, db_path) = testutil::temp_db();
    let router = router_over(&db_path);

    let request = tauri::http::Request::builder()
        .method("GET")
        .uri("nigel://evil.example/")
        .body(Vec::new())
        .expect("build scheme request");

    let response = nigel_desktop::transport::answer(router, request).await;

    assert_eq!(response.status(), 403);
}
