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

#[tokio::test]
async fn until_roles_land_a_non_admin_reaches_only_their_own_account() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let sam = add_ready_user(&db_path, "sam@example.com", Role::Bookkeeper);
    let token = session_for(
        &db_path,
        sam.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    for uri in [
        "/api/status",
        "/api/accounts",
        "/api/reports/pnl",
        "/api/no-such-route",
    ] {
        let response = app.clone().oneshot(bearer_get(uri, &token)).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
        let json = json_body(response).await;
        assert_eq!(json["error"]["details"]["reason"], "role", "{uri}");
        assert_eq!(json["error"]["details"]["required"], "admin", "{uri}");
    }
    let ping = app.oneshot(bearer_get("/api/ping", &token)).await.unwrap();
    assert_eq!(ping.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_admin_reaches_the_data_routes_and_status_reports_shared() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    let accounts = app
        .clone()
        .oneshot(bearer_get("/api/accounts", &token))
        .await
        .unwrap();
    assert_eq!(accounts.status(), StatusCode::OK);
    let status = app
        .oneshot(bearer_get("/api/status", &token))
        .await
        .unwrap();
    assert_eq!(json_body(status).await["mode"], "shared");
}

#[tokio::test]
async fn unlocking_books_that_have_users_ends_the_token_session() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    encrypt(&db_path);
    crate::db::set_db_password_for_tests(None);
    let (app, state) = served_app(&db_path, public_config());
    assert_eq!(state.mode(), super::mode::Mode::Local);
    let token = state.local_token().unwrap().expose().to_string();

    let unlocked = app
        .clone()
        .oneshot(session_post("/api/unlock", &token, &unlock_body(PASSWORD)))
        .await
        .unwrap();
    assert_eq!(unlocked.status(), StatusCode::OK);
    assert_eq!(state.mode(), super::mode::Mode::Shared);

    let next = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(next.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(next).await["error"]["details"]["mode"], "shared");
    crate::db::set_db_password_for_tests(None);
}

#[tokio::test]
async fn switching_to_a_folder_whose_books_have_users_ends_the_token_session() {
    let _config = TempConfig::new();
    let (_dir, db_path) = temp_db();
    let (app, state) = served_app(&db_path, public_config());
    assert_eq!(state.mode(), super::mode::Mode::Local);
    let token = state.local_token().unwrap().expose().to_string();

    let (_other_dir, other_db) = temp_db();
    add_ready_user(&other_db, "pat@example.com", Role::Admin);
    let body = serde_json::json!({ "path": other_db.parent().unwrap().to_string_lossy() });
    let switched = app
        .clone()
        .oneshot(session_post(
            "/api/settings/data-dir",
            &token,
            &body.to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(switched.status(), StatusCode::OK);
    assert_eq!(json_body(switched).await["mode"], "shared");
    assert_eq!(state.mode(), super::mode::Mode::Shared);

    let next = app.oneshot(session_get("/api/ping", &token)).await.unwrap();
    assert_eq!(next.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(next).await["error"]["details"]["mode"], "shared");
}

use axum::extract::ConnectInfo;
use std::net::SocketAddr;

fn login_request(body: &str, origin: Option<&str>, peer: &str) -> Request<Body> {
    let mut builder =
        public_request("POST", "/api/auth/login").header(header::CONTENT_TYPE, "application/json");
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    let mut request = builder.body(Body::from(body.to_owned())).unwrap();
    let peer: SocketAddr = peer.parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(peer));
    request
}

fn login_body(email: &str, password: &str) -> String {
    serde_json::json!({ "email": email, "password": password }).to_string()
}

const PEER: &str = "198.51.100.7:50000";

#[tokio::test]
async fn a_browser_login_sets_a_secure_cookie_on_the_public_host() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());

    let response = app
        .clone()
        .oneshot(login_request(
            &login_body("Pat@Example.com", READY_PASSWORD),
            Some(PUBLIC_URL),
            PEER,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = header_str(&response, header::SET_COOKIE);
    for part in [
        "nigel_session=",
        "HttpOnly",
        "SameSite=Strict",
        "Path=/",
        "Max-Age=2592000",
        "Secure",
    ] {
        assert!(cookie.contains(part), "{part}: {cookie}");
    }
    let json = json_body(response).await;
    assert_eq!(json["user"]["email"], "pat@example.com");
    assert_eq!(json["user"]["role"], "admin");
    assert!(json.get("token").is_none());

    let token = cookie
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("nigel_session=")
        .to_string();
    let me = app
        .oneshot(cookie_get("/api/auth/me", &token))
        .await
        .unwrap();
    let me = json_body(me).await;
    assert_eq!(me["mode"], "shared");
    assert_eq!(me["role"], "admin");
    assert_eq!(me["user"]["displayName"], "Pat Example");
    assert_eq!(me["user"]["mustChangePassword"], false);
}

#[tokio::test]
async fn a_loopback_login_cookie_is_not_secure() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let mut request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, "http://127.0.0.1:5731")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(login_body("pat@example.com", READY_PASSWORD)))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:50000".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = header_str(&response, header::SET_COOKIE);
    assert!(cookie.contains("nigel_session="), "{cookie}");
    assert!(!cookie.contains("Secure"), "{cookie}");
}

#[tokio::test]
async fn a_native_login_answers_a_bearer_token_and_sets_no_cookie() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let body = serde_json::json!({
        "email": "pat@example.com", "password": READY_PASSWORD, "client": "native"
    })
    .to_string();
    let response = app
        .clone()
        .oneshot(login_request(&body, None, PEER))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_str(&response, header::SET_COOKIE), "");
    let json = json_body(response).await;
    let token = json["token"].as_str().unwrap().to_string();
    assert_eq!(token.len(), 64);
    assert!(json["expiresAt"].as_str().unwrap().ends_with('Z'));
    let ping = app.oneshot(bearer_get("/api/ping", &token)).await.unwrap();
    assert_eq!(ping.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_browser_login_without_an_origin_is_refused() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let response = app
        .oneshot(login_request(
            &login_body("pat@example.com", READY_PASSWORD),
            None,
            PEER,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(header_str(&response, header::SET_COOKIE), "");
}

#[tokio::test]
async fn wrong_unknown_and_disabled_credentials_all_answer_alike() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let sam = add_ready_user(&db_path, "sam@example.com", Role::Admin);
    let conn = crate::db::get_connection(&db_path).unwrap();
    crate::identity::users::disable_user(&conn, sam.id, crate::clock::now_utc()).unwrap();
    let (app, _state) = served_app(&db_path, public_config());

    let mut answers = Vec::new();
    for (email, password, peer) in [
        (
            "pat@example.com",
            "not the password at all",
            "198.51.100.1:1",
        ),
        ("nobody@example.com", READY_PASSWORD, "198.51.100.2:1"),
        ("sam@example.com", READY_PASSWORD, "198.51.100.3:1"),
        ("not an email", READY_PASSWORD, "198.51.100.4:1"),
    ] {
        let response = app
            .clone()
            .oneshot(login_request(
                &login_body(email, password),
                Some(PUBLIC_URL),
                peer,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{email}");
        answers.push(body_string(response).await);
    }
    assert!(answers.windows(2).all(|w| w[0] == w[1]), "{answers:?}");
    let json: serde_json::Value = serde_json::from_str(&answers[0]).unwrap();
    assert_eq!(json["error"]["code"], "invalid_credentials");
    assert_eq!(
        json["error"]["message"],
        "That email and password don't match."
    );
}

#[tokio::test]
async fn five_wrong_passwords_lock_the_account_before_the_password_is_checked() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    for _ in 0..5 {
        let wrong = app
            .clone()
            .oneshot(login_request(
                &login_body("pat@example.com", "wrong password here"),
                Some(PUBLIC_URL),
                PEER,
            ))
            .await
            .unwrap();
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    }
    let locked = app
        .oneshot(login_request(
            &login_body("pat@example.com", READY_PASSWORD),
            Some(PUBLIC_URL),
            "198.51.100.99:1",
        ))
        .await
        .unwrap();
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);
    let json = json_body(locked).await;
    assert_eq!(json["error"]["code"], "too_many_attempts");
    assert_eq!(
        json["error"]["message"],
        "Sign-in is temporarily unavailable. Try again later."
    );
    let wait = json["error"]["details"]["retryAfterMs"].as_u64().unwrap();
    assert!(wait > 0 && wait <= 15 * 60 * 1000, "{wait}");
}

/// Two-step for pat, sealed under a server key in a temporary config dir
/// that the caller keeps alive.
fn enrol_totp(db_path: &std::path::Path, user_id: i64) -> [u8; 20] {
    use crate::identity::{seal, totp, users};
    let key = seal::ServerKey::load_or_create(&seal::key_path()).unwrap();
    let secret = [7u8; 20];
    let now = crate::clock::now_utc();
    let conn = crate::db::get_connection(db_path).unwrap();
    users::set_totp_secret(&conn, user_id, &key, &secret, totp::step_at(now) - 5, now).unwrap();
    secret
}

#[tokio::test]
async fn a_second_factor_is_asked_for_then_checked() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let _config = crate::settings::TempConfigDir::new();
    let secret = enrol_totp(&db_path, pat.id);
    let (app, _state) = served_app(&db_path, public_config());

    let needs_code = app
        .clone()
        .oneshot(login_request(
            &login_body("pat@example.com", READY_PASSWORD),
            Some(PUBLIC_URL),
            PEER,
        ))
        .await
        .unwrap();
    assert_eq!(needs_code.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(needs_code).await["error"]["code"],
        "totp_required"
    );

    let wrong = serde_json::json!({
        "email": "pat@example.com", "password": READY_PASSWORD, "code": "000000"
    })
    .to_string();
    let wrong = app
        .clone()
        .oneshot(login_request(&wrong, Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    let wrong = json_body(wrong).await;
    assert_eq!(wrong["error"]["code"], "invalid_credentials");
    assert_eq!(
        wrong["error"]["message"],
        "That code didn't match. Check the time on your device."
    );

    let code = crate::identity::totp::code_for_step(
        &secret,
        crate::identity::totp::step_at(crate::clock::now_utc()),
    );
    let right = serde_json::json!({
        "email": "pat@example.com", "password": READY_PASSWORD, "code": code
    })
    .to_string();
    let right = app
        .oneshot(login_request(&right, Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    assert_eq!(right.status(), StatusCode::OK);
}

#[tokio::test]
async fn being_asked_for_a_code_is_not_a_failed_attempt() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let _config = crate::settings::TempConfigDir::new();
    let secret = enrol_totp(&db_path, pat.id);
    let (app, _state) = served_app(&db_path, public_config());

    for _ in 0..6 {
        let needs_code = app
            .clone()
            .oneshot(login_request(
                &login_body("pat@example.com", READY_PASSWORD),
                Some(PUBLIC_URL),
                PEER,
            ))
            .await
            .unwrap();
        assert_eq!(
            json_body(needs_code).await["error"]["code"],
            "totp_required"
        );
    }
    let code = crate::identity::totp::code_for_step(
        &secret,
        crate::identity::totp::step_at(crate::clock::now_utc()),
    );
    let right = serde_json::json!({
        "email": "pat@example.com", "password": READY_PASSWORD, "code": code
    })
    .to_string();
    let right = app
        .oneshot(login_request(&right, Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    assert_eq!(right.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_second_factor_this_server_cannot_check_is_not_a_failed_attempt() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let config = crate::settings::TempConfigDir::new();
    enrol_totp(&db_path, pat.id);
    drop(config);
    let _other_config = crate::settings::TempConfigDir::new();
    let (app, _state) = served_app(&db_path, public_config());

    let with_code = serde_json::json!({
        "email": "pat@example.com", "password": READY_PASSWORD, "code": "123456"
    })
    .to_string();
    for _ in 0..6 {
        let unavailable = app
            .clone()
            .oneshot(login_request(&with_code, Some(PUBLIC_URL), PEER))
            .await
            .unwrap();
        assert_eq!(unavailable.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let json = json_body(unavailable).await;
        assert_eq!(json["error"]["details"]["reason"], "totp_unavailable");
    }
}

#[tokio::test]
async fn an_oversized_login_body_is_refused_before_any_check() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let body = login_body("pat@example.com", &"x".repeat(9 * 1024));
    let response = app
        .oneshot(login_request(&body, Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!body_string(response).await.contains("xxxx"));
}

#[tokio::test]
async fn a_malformed_login_body_quotes_nothing_back() {
    let (_dir, db_path) = temp_db();
    add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let (app, _state) = served_app(&db_path, public_config());
    let body = r#"{"email":"pat@example.com","password":12345678901}"#;
    let response = app
        .oneshot(login_request(body, Some(PUBLIC_URL), PEER))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!body_string(response).await.contains("12345678901"));
}

#[tokio::test]
async fn logout_ends_the_session_and_expires_the_cookie() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(
        &db_path,
        pat.id,
        SessionKind::Cookie,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());
    let response = app
        .clone()
        .oneshot(
            public_request("POST", "/api/auth/logout")
                .header(header::COOKIE, format!("nigel_session={token}"))
                .header(header::ORIGIN, PUBLIC_URL)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header_str(&response, header::SET_COOKIE).contains("Max-Age=0"));
    assert_eq!(json_body(response).await["loggedOut"], true);
    let after = app.oneshot(cookie_get("/api/ping", &token)).await.unwrap();
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sign_in_routes_answer_while_the_books_are_locked() {
    let (_dir, db_path) = temp_db();
    encrypt(&db_path);
    crate::db::set_db_password_for_tests(None);
    let (app, state) = served_app(&db_path, public_config());
    let token = state.local_token().unwrap().expose().to_string();
    let me = app
        .clone()
        .oneshot(session_get("/api/auth/me", &token))
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let logout = app
        .oneshot(session_post("/api/auth/logout", &token, ""))
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::OK);
}

fn bearer_post(uri: &str, token: &str, body: serde_json::Value) -> Request<Body> {
    public_request("POST", uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn a_temporary_password_is_changed_and_other_sessions_end() {
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
    let this = session_for(
        &db_path,
        created.user.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let other = session_for(
        &db_path,
        created.user.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    for (new, reason) in [
        ("short", "too_short"),
        ("pat@example.com", "matches_identity"),
    ] {
        let response = app
            .clone()
            .oneshot(bearer_post(
                "/api/account/password",
                &this,
                serde_json::json!({
                    "currentPassword": created.temporary_password.as_str(), "newPassword": new
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{new}");
        assert_eq!(
            json_body(response).await["error"]["details"]["reason"],
            reason
        );
    }
    let wrong = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/password",
            &this,
            serde_json::json!({
                "currentPassword": "not it at all", "newPassword": "a brand new passphrase"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(wrong).await["error"]["details"]["reason"],
        "wrong_password"
    );

    let changed = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/password",
            &this,
            serde_json::json!({
                "currentPassword": created.temporary_password.as_str(),
                "newPassword": "a brand new passphrase"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::OK);

    let accounts = app
        .clone()
        .oneshot(bearer_get("/api/accounts", &this))
        .await
        .unwrap();
    assert_eq!(
        accounts.status(),
        StatusCode::OK,
        "must-change cleared, this session kept"
    );
    let ended = app.oneshot(bearer_get("/api/ping", &other)).await.unwrap();
    assert_eq!(ended.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn two_step_is_set_up_confirmed_and_removed() {
    let (_dir, db_path) = temp_db();
    let _config = TempConfig::new();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());

    let wrong_password = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/setup",
            &token,
            serde_json::json!({ "currentPassword": "nope nope nope" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(wrong_password).await["error"]["details"]["reason"],
        "wrong_password"
    );

    let setup = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/setup",
            &token,
            serde_json::json!({ "currentPassword": READY_PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(setup.status(), StatusCode::OK);
    assert_eq!(
        header_str(&setup, header::CACHE_CONTROL),
        "no-store",
        "the secret is never cached"
    );
    let setup = json_body(setup).await;
    let setup_id = setup["setupId"].as_str().unwrap().to_string();
    let secret_b32 = setup["secret"].as_str().unwrap();
    assert!(setup["otpauthUri"]
        .as_str()
        .unwrap()
        .starts_with("otpauth://totp/Nigel:pat@example.com?secret="));
    assert!(setup["qrSvg"].as_str().unwrap().contains("<svg"));
    let secret = data_encoding::BASE32_NOPAD
        .decode(secret_b32.as_bytes())
        .unwrap();
    assert!(
        !crate::identity::users::get_user(&crate::db::get_connection(&db_path).unwrap(), pat.id)
            .unwrap()
            .unwrap()
            .totp_enabled,
        "nothing stored yet"
    );

    let bad = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/confirm",
            &token,
            serde_json::json!({ "setupId": setup_id, "code": "000000" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(bad).await["error"]["details"]["reason"],
        "wrong_code"
    );

    let stale = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/confirm",
            &token,
            serde_json::json!({ "setupId": "ff".repeat(16), "code": "000000" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(stale).await["error"]["details"]["reason"],
        "setup_expired"
    );

    let now = crate::clock::now_utc();
    let step = crate::identity::totp::step_at(now);
    let code = crate::identity::totp::code_for_step(&secret, step);
    let confirmed = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/confirm",
            &token,
            serde_json::json!({ "setupId": setup_id, "code": code }),
        ))
        .await
        .unwrap();
    assert_eq!(confirmed.status(), StatusCode::OK);
    assert!(
        crate::identity::seal::key_path().exists(),
        "server.key created on first enrollment"
    );

    let again = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/setup",
            &token,
            serde_json::json!({ "currentPassword": READY_PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(
        json_body(again).await["error"]["details"]["reason"],
        "totp_enabled"
    );

    let next = crate::identity::totp::code_for_step(&secret, step + 1);
    let removed = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/remove",
            &token,
            serde_json::json!({ "currentPassword": READY_PASSWORD, "code": next }),
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    let conn = crate::db::get_connection(&db_path).unwrap();
    assert!(
        !crate::identity::users::get_user(&conn, pat.id)
            .unwrap()
            .unwrap()
            .totp_enabled
    );
}

#[tokio::test]
async fn account_routes_are_a_conflict_for_the_local_owner() {
    let (_dir, db_path) = temp_db();
    let (app, token) = app_for(&db_path);
    let response = app
        .oneshot(session_post(
            "/api/account/password",
            &token,
            r#"{"currentPassword":"x","newPassword":"y"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_body(response).await["error"]["details"]["reason"],
        "local_mode"
    );
}

#[tokio::test]
async fn wrong_current_passwords_on_an_account_route_lock_like_logins() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());
    for _ in 0..5 {
        app.clone()
            .oneshot(bearer_post(
                "/api/account/totp/setup",
                &token,
                serde_json::json!({ "currentPassword": "wrong password here" }),
            ))
            .await
            .unwrap();
    }
    let locked = app
        .oneshot(bearer_post(
            "/api/account/totp/setup",
            &token,
            serde_json::json!({ "currentPassword": READY_PASSWORD }),
        ))
        .await
        .unwrap();
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);
    let locked = json_body(locked).await;
    assert_eq!(
        locked["error"]["message"],
        "Too many wrong passwords. Try again later."
    );
    let wait = locked["error"]["details"]["retryAfterMs"].as_u64().unwrap();
    assert!(wait > 0 && wait <= 15 * 60 * 1000, "{wait}");
}

#[tokio::test]
async fn wrong_codes_when_removing_two_step_lock_like_logins() {
    let (_dir, db_path) = temp_db();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let _config = TempConfig::new();
    let secret = enrol_totp(&db_path, pat.id);
    let token = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());
    let right = crate::identity::totp::code_for_step(
        &secret,
        crate::identity::totp::step_at(crate::clock::now_utc()),
    );
    let wrong = if right == "000000" {
        "111111"
    } else {
        "000000"
    };
    for _ in 0..5 {
        let response = app
            .clone()
            .oneshot(bearer_post(
                "/api/account/totp/remove",
                &token,
                serde_json::json!({ "currentPassword": READY_PASSWORD, "code": wrong }),
            ))
            .await
            .unwrap();
        assert_eq!(
            json_body(response).await["error"]["details"]["reason"],
            "wrong_code"
        );
    }
    let locked = app
        .oneshot(bearer_post(
            "/api/account/totp/remove",
            &token,
            serde_json::json!({ "currentPassword": READY_PASSWORD, "code": right }),
        ))
        .await
        .unwrap();
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_server_key_that_cannot_be_read_is_a_clean_totp_unavailable() {
    let (_dir, db_path) = temp_db();
    let _config = TempConfig::new();
    let pat = add_ready_user(&db_path, "pat@example.com", Role::Admin);
    let token = session_for(
        &db_path,
        pat.id,
        SessionKind::Bearer,
        crate::clock::now_utc(),
    );
    let (app, _state) = served_app(&db_path, public_config());
    let key_path = crate::identity::seal::key_path();
    std::fs::create_dir_all(key_path.parent().unwrap()).unwrap();
    std::fs::write(&key_path, b"not a key").unwrap();

    let setup = app
        .clone()
        .oneshot(bearer_post(
            "/api/account/totp/setup",
            &token,
            serde_json::json!({ "currentPassword": READY_PASSWORD }),
        ))
        .await
        .unwrap();
    let setup = json_body(setup).await;
    let secret = data_encoding::BASE32_NOPAD
        .decode(setup["secret"].as_str().unwrap().as_bytes())
        .unwrap();
    let code = crate::identity::totp::code_for_step(
        &secret,
        crate::identity::totp::step_at(crate::clock::now_utc()),
    );
    let confirmed = app
        .oneshot(bearer_post(
            "/api/account/totp/confirm",
            &token,
            serde_json::json!({ "setupId": setup["setupId"], "code": code }),
        ))
        .await
        .unwrap();
    assert_eq!(confirmed.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = json_body(confirmed).await;
    assert_eq!(body["error"]["details"]["reason"], "totp_unavailable");
    let message = body["error"]["message"].as_str().unwrap();
    assert!(!message.contains("server.key"), "{message}");
    assert!(!message.contains(&code), "{message}");
    assert_eq!(
        std::fs::read(&key_path).unwrap(),
        b"not a key",
        "left as it is"
    );
}
