//! Shared-mode behaviour of the served router, end to end through tower.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use tower::ServiceExt;

use super::testutil::*;
use crate::identity::Role;

fn status_of(response: &axum::response::Response) -> StatusCode {
    response.status()
}

#[tokio::test]
async fn in_local_mode_the_public_host_is_refused_and_loopback_is_not() {
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    let token = state.local_token().unwrap().expose().to_string();

    let public = app
        .clone()
        .oneshot(
            public_request("GET", "/api/ping")
                .header(header::COOKIE, format!("nigel_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_of(&public), StatusCode::FORBIDDEN);

    let loopback = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(status_of(&loopback), StatusCode::OK);
}

#[tokio::test]
async fn in_shared_mode_the_public_host_passes_and_no_other_name_does() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());

    let public = app
        .clone()
        .oneshot(
            public_request("GET", "/api/ping")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        public.status(),
        StatusCode::UNAUTHORIZED,
        "past the host guard"
    );

    for host in [
        "other.example.com",
        "books.example.com.evil.com",
        "books.example.com:8443",
    ] {
        let refused = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/ping")
                    .header(header::HOST, host)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::FORBIDDEN, "{host}");
    }
}

#[tokio::test]
async fn the_public_origin_is_trusted_whole_and_only_in_shared_mode() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    for (origin, expected) in [
        ("https://books.example.com", StatusCode::UNAUTHORIZED),
        ("http://localhost:5173", StatusCode::UNAUTHORIZED),
        ("http://books.example.com", StatusCode::FORBIDDEN),
        ("https://books.example.com:8443", StatusCode::FORBIDDEN),
        ("https://evil.example.com", StatusCode::FORBIDDEN),
    ] {
        let response = app
            .clone()
            .oneshot(
                public_request("GET", "/api/ping")
                    .header(header::ORIGIN, origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{origin}");
    }

    let (_dir2, local_db) = temp_db();
    let (local_app, state) = served_app(&local_db, public_config());
    let token = state.local_token().unwrap().expose().to_string();
    let response = local_app
        .oneshot(
            get_request("/api/ping")
                .header(header::COOKIE, format!("nigel_session={token}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "local mode ignores the public origin"
    );
}

#[tokio::test]
async fn a_first_user_added_to_a_running_local_server_takes_effect_on_the_next_public_request() {
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    assert_eq!(state.mode(), super::mode::Mode::Local);

    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let response = app
        .oneshot(
            public_request("GET", "/api/ping")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(state.mode(), super::mode::Mode::Shared);
    assert!(state.local_token().is_none(), "the token is dropped");
}

#[tokio::test]
async fn hsts_and_no_store_go_where_they_belong() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());

    let public_api = app
        .clone()
        .oneshot(
            public_request("GET", "/api/ping")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        header_str(&public_api, header::STRICT_TRANSPORT_SECURITY),
        "max-age=31536000"
    );
    assert_eq!(header_str(&public_api, header::CACHE_CONTROL), "no-store");
    assert_eq!(
        header_str(&public_api, header::REFERRER_POLICY),
        "same-origin"
    );

    let loopback_shell = app
        .clone()
        .oneshot(get_request("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        header_str(&loopback_shell, header::STRICT_TRANSPORT_SECURITY),
        ""
    );
    assert_eq!(
        header_str(&loopback_shell, header::REFERRER_POLICY),
        "same-origin"
    );
    assert_ne!(
        header_str(&loopback_shell, header::CACHE_CONTROL),
        "",
        "the shell keeps its own caching header"
    );

    let loopback_api = app
        .oneshot(get_request("/api/ping").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(
        header_str(&loopback_api, header::STRICT_TRANSPORT_SECURITY),
        ""
    );
    assert_eq!(header_str(&loopback_api, header::CACHE_CONTROL), "no-store");
}
