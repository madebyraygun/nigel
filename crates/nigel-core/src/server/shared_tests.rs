//! Shared-mode behaviour of the served router, end to end through tower.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use chrono::{DateTime, Duration, Utc};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

use super::state::AppState;
use super::testutil::*;
use crate::identity::sessions::SessionKind;
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

fn t0() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

/// A served router over `db_path` whose clock reads whatever `clock` holds.
fn served_at(
    db_path: &std::path::Path,
    clock: Arc<Mutex<DateTime<Utc>>>,
) -> (axum::Router, AppState) {
    let mode = super::mode::derive(db_path).unwrap();
    let state = AppState::for_serve(db_path.to_path_buf(), mode, public_config())
        .with_clock(move || *clock.lock().unwrap());
    (super::build_router(state.clone()), state)
}

fn cookie_get(uri: &str, token: &str) -> Request<Body> {
    public_request("GET", uri)
        .header(header::COOKIE, format!("nigel_session={token}"))
        .body(Body::empty())
        .unwrap()
}

fn bearer_get(uri: &str, token: &str) -> Request<Body> {
    public_request("GET", uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn a_cookie_or_bearer_session_signs_a_request_in() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let cookie = session_for(
        &db_path,
        pat.id,
        SessionKind::Cookie,
        crate::clock::now_utc(),
    );
    let bearer = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    for request in [
        cookie_get("/api/ping", &cookie),
        bearer_get("/api/ping", &bearer),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let none = app
        .clone()
        .oneshot(
            public_request("GET", "/api/ping")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(none.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(none).await["error"]["details"]["mode"], "shared");

    let unknown = app
        .oneshot(cookie_get("/api/ping", &"0".repeat(64)))
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_session_answers_only_on_its_own_transport() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let cookie = session_for(
        &db_path,
        pat.id,
        SessionKind::Cookie,
        crate::clock::now_utc(),
    );
    let bearer = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    for request in [
        bearer_get("/api/ping", &cookie),
        cookie_get("/api/ping", &bearer),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let malformed = app
        .oneshot(
            public_request("GET", "/api/ping")
                .header(header::AUTHORIZATION, "Basic cGF0OnBhc3M=")
                .header(header::COOKIE, format!("nigel_session={cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        malformed.status(),
        StatusCode::UNAUTHORIZED,
        "no fallback to the cookie"
    );
}

#[tokio::test]
async fn the_old_token_cookie_is_refused_once_shared() {
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    let token = state.local_token().unwrap().expose().to_string();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    assert_eq!(state.refresh_mode().await, super::mode::Mode::Shared);

    let response = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(response).await["error"]["details"]["mode"],
        "shared"
    );
}

#[tokio::test]
async fn a_loopback_token_request_rederives_the_mode_once_the_throttle_allows() {
    let (_dir, db_path) = temp_db();
    let clock = Arc::new(Mutex::new(t0()));
    let (app, state) = served_at(&db_path, clock.clone());
    let token = state.local_token().unwrap().expose().to_string();

    let first = app
        .clone()
        .oneshot(session_get("/api/ping", &token))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    *clock.lock().unwrap() = t0() + Duration::seconds(1);
    let throttled = app
        .clone()
        .oneshot(session_get("/api/ping", &token))
        .await
        .unwrap();
    assert_eq!(throttled.status(), StatusCode::OK, "within the throttle");

    *clock.lock().unwrap() = t0() + Duration::seconds(3);
    let refused = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(refused).await["error"]["details"]["mode"],
        "shared"
    );
    assert_eq!(state.mode(), super::mode::Mode::Shared);
}

#[tokio::test]
async fn a_cookie_write_needs_an_origin_and_a_bearer_write_does_not() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let cookie = session_for(
        &db_path,
        pat.id,
        SessionKind::Cookie,
        crate::clock::now_utc(),
    );
    let bearer = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());
    let body = r#"{"name":"Acme Checking","accountType":"checking"}"#;

    let no_origin = app
        .clone()
        .oneshot(
            public_request("POST", "/api/accounts")
                .header(header::COOKIE, format!("nigel_session={cookie}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_origin.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json_body(no_origin).await["error"]["details"]["reason"],
        "origin_required"
    );

    let with_origin = app
        .clone()
        .oneshot(
            public_request("POST", "/api/accounts")
                .header(header::COOKIE, format!("nigel_session={cookie}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(with_origin.status(), StatusCode::FORBIDDEN);
    assert_ne!(with_origin.status(), StatusCode::UNAUTHORIZED);

    let via_bearer = app
        .oneshot(
            public_request("POST", "/api/accounts")
                .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"name":"Acme Card","accountType":"checking"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(via_bearer.status(), StatusCode::FORBIDDEN);
    assert_ne!(via_bearer.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_user_who_must_change_their_password_reaches_only_the_allowlist() {
    let (_dir, db_path) = temp_db();
    let conn = crate::db::get_connection(&db_path).unwrap();
    let created = crate::identity::users::create_user(
        &conn,
        &crate::identity::users::NewUser {
            email: "pat@example.com",
            display_name: "Pat Example",
            role: Role::Admin,
        },
        crate::clock::now_utc(),
    )
    .unwrap();
    let token = session_for(
        &db_path,
        created.user.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    for uri in ["/api/ping", "/api/status"] {
        let response = app.clone().oneshot(bearer_get(uri, &token)).await.unwrap();
        assert_ne!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
    let refused = app
        .oneshot(bearer_get("/api/accounts", &token))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        json_body(refused).await["error"]["code"],
        "password_change_required"
    );
}

#[tokio::test]
async fn renewal_slides_thirty_days_hourly_and_stops_at_ninety() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Cookie, t0());
    let clock = Arc::new(Mutex::new(t0()));
    let (app, _state) = served_at(&db_path, clock.clone());
    let set_cookie_at = |days: i64| {
        *clock.lock().unwrap() = t0() + Duration::days(days);
        let app = app.clone();
        let token = token.clone();
        async move {
            let response = app.oneshot(cookie_get("/api/ping", &token)).await.unwrap();
            (response.status(), header_str(&response, header::SET_COOKIE))
        }
    };

    let (status, cookie) = set_cookie_at(0).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cookie, "", "used within the hour: no renewal");

    let (_, cookie) = set_cookie_at(20).await;
    assert!(cookie.contains("Max-Age=2592000"), "{cookie}");
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"),
        "{cookie}"
    );
    assert!(cookie.contains("Secure"), "public host: {cookie}");

    set_cookie_at(45).await;
    let (_, cookie) = set_cookie_at(70).await;
    assert!(
        cookie.contains(&format!("Max-Age={}", 20 * 86_400)),
        "capped at 90 days: {cookie}"
    );

    let (status, _) = set_cookie_at(90).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_renewed_cookie_on_loopback_is_not_secure() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Cookie, t0());
    let clock = Arc::new(Mutex::new(t0() + Duration::days(2)));
    let (app, _state) = served_at(&db_path, clock);
    let response = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = header_str(&response, header::SET_COOKIE);
    assert!(cookie.contains("Max-Age=2592000"), "{cookie}");
    assert!(!cookie.contains("Secure"), "loopback: {cookie}");
}

#[tokio::test]
async fn an_idle_session_expires_after_thirty_days() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(&db_path, pat.id, SessionKind::Bearer, t0());
    let clock = Arc::new(Mutex::new(t0() + Duration::days(30)));
    let (app, _state) = served_at(&db_path, clock);
    let response = app.oneshot(bearer_get("/api/ping", &token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disabling_a_user_ends_their_next_request() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let sam = add_ready_user(&db_path, "sam@example.com", Role::Admin);
    let token = session_for(
        &db_path,
        sam.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());
    assert_eq!(
        app.clone()
            .oneshot(bearer_get("/api/ping", &token))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let conn = crate::db::get_connection(&db_path).unwrap();
    crate::identity::users::disable_user(&conn, sam.id, crate::clock::now_utc()).unwrap();
    assert_eq!(
        app.oneshot(bearer_get("/api/ping", &token))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
