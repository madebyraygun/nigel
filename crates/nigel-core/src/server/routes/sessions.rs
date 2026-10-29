//! Signing in and out: `POST /api/auth/login`, `POST /api/auth/logout`,
//! `GET /api/auth/me`. Login is the one `/api` route that needs no session;
//! all three answer while the books are locked.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::identity::seal::{self, ServerKey};
use crate::identity::sessions::{self, IssuedSession, SessionKind};
use crate::identity::users::{self, Credentials, SignIn, User};
use crate::identity::Role;

use super::super::auth::{expired_cookie, request_is_public, session_cookie};
use super::super::error::{ApiError, ApiResult};
use super::super::events::{emit, AuthEvent, EventKind};
use super::super::extract::{PeerAddr, SecretJson};
use super::super::mode::Mode;
use super::super::principal::Principal;
use super::super::ratelimit::{account_key, client_ip, Attempt, Bucket, ClientIp, LOCKOUT};
use super::super::secret::Secret;
use super::super::state::AppState;

/// The most a login body may be. A password is at most 256 characters.
pub const LOGIN_BODY_LIMIT: usize = 8 * 1024;

/// The least time between two `limiter_full` lines.
const FULL_NOTICE_EVERY: Duration = Duration::from_secs(60);

static LAST_FULL_NOTICE: Mutex<Option<Instant>> = Mutex::new(None);

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/auth/login",
            post(login).layer(DefaultBodyLimit::max(LOGIN_BODY_LIMIT)),
        )
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LoginClient {
    #[default]
    Browser,
    Native,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    email: String,
    password: Secret,
    #[serde(default)]
    code: Option<Secret>,
    #[serde(default)]
    client: LoginClient,
}

/// A user as the API shows one. No hash, no secret.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UserBody {
    id: i64,
    email: String,
    display_name: String,
    role: Role,
    totp_enabled: bool,
    must_change_password: bool,
}

impl From<&User> for UserBody {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: user.role,
            totp_enabled: user.totp_enabled,
            must_change_password: user.must_change_password,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    user: UserBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
}

enum Outcome {
    BadPassword,
    CodeRequired,
    BadCode(User),
    TotpUnavailable(User),
    SignedIn(User, IssuedSession),
}

/// `retryAfterMs` for a running lockout: never more than one lockout's
/// length, and never zero.
pub(crate) fn retry_after_ms(wait: Duration) -> u128 {
    wait.min(LOCKOUT).as_millis().max(1)
}

/// Whether a `limiter_full` line is due, recording it when it is.
fn full_notice_due(last: &Mutex<Option<Instant>>, now: Instant) -> bool {
    // unwrap: poisoned mutex means a thread panicked — unrecoverable
    let mut last = last.lock().unwrap();
    let due = last.is_none_or(|at| now.duration_since(at) >= FULL_NOTICE_EVERY);
    if due {
        *last = Some(now);
    }
    due
}

/// Tell the operator a limiter table filled with lockouts and began
/// admitting new keys untracked: at most one line a minute.
pub(super) fn report_full(attempt: &Attempt) {
    let full = attempt.newly_full();
    if full.is_empty() || !full_notice_due(&LAST_FULL_NOTICE, Instant::now()) {
        return;
    }
    for bucket in full {
        emit(&AuthEvent::new(EventKind::LimiterFull).reason(bucket.as_str()));
    }
}

/// Whether a user row, enabled or not, has this address. Normalized as
/// `account_key` normalizes it; anything that is not an address matches no one.
fn is_known_user(conn: &Connection, email: &str) -> crate::error::Result<bool> {
    if users::normalize_email(email).is_err() {
        return Ok(false);
    }
    Ok(users::find_by_email(conn, email)?.is_some())
}

fn emit_failure(
    ip: &ClientIp,
    email: &str,
    user_id: Option<i64>,
    reason: &'static str,
    locked: Vec<Bucket>,
) {
    let event = |kind| {
        let event = AuthEvent::new(kind).email(email).ip(&ip.log);
        match user_id {
            Some(id) => event.user(id),
            None => event,
        }
    };
    emit(&event(EventKind::LoginFailed).reason(reason));
    for bucket in locked {
        emit(&event(EventKind::Lockout).reason(bucket.as_str()));
    }
}

async fn login(
    State(state): State<AppState>,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<LoginRequest>, ApiError>,
) -> ApiResult<Response> {
    if state.refresh_mode_throttled().await != Mode::Shared {
        return Err(ApiError::local_mode());
    }
    let SecretJson(request) = body?;
    let client = request.client;
    if client == LoginClient::Browser && !headers.contains_key(header::ORIGIN) {
        return Err(ApiError::origin_required());
    }

    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    let account = account_key(&request.email);

    let lookup_email = request.email.clone();
    let known_user =
        super::with_conn(&state, move |conn| is_known_user(conn, &lookup_email)).await?;

    let attempt = match state
        .limiter
        .begin(&account, known_user, &ip.bucket, Instant::now())
    {
        Ok(attempt) => attempt,
        Err(wait) => {
            emit(
                &AuthEvent::new(EventKind::LoginRefused)
                    .email(&account)
                    .ip(&ip.log),
            );
            return Err(ApiError::too_many_attempts(retry_after_ms(wait)));
        }
    };
    report_full(&attempt);
    let Some(permit) = state.password_check().await else {
        state.limiter.not_a_failure(attempt);
        return Err(ApiError::busy());
    };

    let kind = match client {
        LoginClient::Browser => SessionKind::Cookie,
        LoginClient::Native => SessionKind::Bearer,
    };
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let now = state.now();
    let row_ip = ip.log.clone();
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let judged = super::spawn_blocking_holding(gate, move || -> crate::error::Result<Outcome> {
        let _permit = permit;
        let conn = crate::db::get_connection(&db_path)?;
        let proof =
            match users::verify_credentials(&conn, &request.email, request.password.expose())? {
                Credentials::Valid(proof) => proof,
                Credentials::Invalid => return Ok(Outcome::BadPassword),
            };
        let key = if proof.user().totp_enabled {
            ServerKey::load(&seal::key_path()).ok().flatten()
        } else {
            None
        };
        let code = request.code.as_ref().map(Secret::expose);
        Ok(
            match users::complete_sign_in(&conn, proof, key.as_ref(), code, now)? {
                SignIn::Granted(user, grant) => {
                    let issued = sessions::create_session(
                        &conn,
                        grant,
                        kind,
                        user_agent.as_deref(),
                        Some(&row_ip),
                        now,
                    )?;
                    sessions::sweep_expired(&conn, now)?;
                    Outcome::SignedIn(user, issued)
                }
                SignIn::CodeRequired(_) => Outcome::CodeRequired,
                SignIn::CodeRejected(user) => Outcome::BadCode(user),
                SignIn::TotpUnavailable(user) => Outcome::TotpUnavailable(user),
            },
        )
    })
    .await;
    let outcome = match judged {
        Ok((_gate, Ok(outcome))) => outcome,
        Ok((_gate, Err(e))) => {
            state.limiter.not_a_failure(attempt);
            return Err(e.into());
        }
        Err(e) => {
            state.limiter.not_a_failure(attempt);
            return Err(e);
        }
    };

    match outcome {
        Outcome::BadPassword => {
            let locked = state.limiter.failed(attempt);
            emit_failure(&ip, &account, None, "credentials", locked);
            Err(ApiError::invalid_credentials())
        }
        Outcome::CodeRequired => {
            state.limiter.not_a_failure(attempt);
            Err(ApiError::totp_required())
        }
        Outcome::BadCode(user) => {
            let locked = state.limiter.failed(attempt);
            emit_failure(&ip, &user.email, Some(user.id), "code", locked);
            Err(ApiError::login_code_mismatch())
        }
        Outcome::TotpUnavailable(user) => {
            state.limiter.not_a_failure(attempt);
            emit(
                &AuthEvent::new(EventKind::TotpUnavailable)
                    .email(&user.email)
                    .user(user.id)
                    .ip(&ip.log),
            );
            Err(ApiError::totp_unavailable())
        }
        Outcome::SignedIn(user, issued) => {
            state.limiter.succeeded(attempt);
            emit(
                &AuthEvent::new(EventKind::LoginSucceeded)
                    .email(&user.email)
                    .user(user.id)
                    .ip(&ip.log)
                    .session(issued.id),
            );
            let native = client == LoginClient::Native;
            let body = LoginResponse {
                user: UserBody::from(&user),
                token: native.then(|| issued.token.to_string()),
                expires_at: native.then(|| issued.expires_at.clone()),
            };
            let mut response = Json(body).into_response();
            if !native {
                let cookie = zeroize::Zeroizing::new(session_cookie(
                    &issued.token,
                    sessions::IDLE_DAYS * 86_400,
                    request_is_public(&state, &headers),
                ));
                response.headers_mut().append(
                    header::SET_COOKIE,
                    HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
                );
            }
            Ok(response)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoggedOut {
    logged_out: bool,
}

/// Ends the current session. In local mode there is none, and the answer is
/// the same.
async fn logout(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let mut response = Json(LoggedOut { logged_out: true }).into_response();
    let Principal::User(user) = principal else {
        return Ok(response);
    };
    let session_id = user.session_id;
    super::with_conn(&state, move |conn| {
        sessions::end_session(conn, session_id).map(|_| ())
    })
    .await?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    emit(
        &AuthEvent::new(EventKind::Logout)
            .email(&user.email)
            .user(user.id)
            .ip(&ip.log)
            .session(session_id),
    );
    if user.kind == SessionKind::Cookie {
        let cookie = expired_cookie(request_is_public(&state, &headers));
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
        );
    }
    Ok(response)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    mode: &'static str,
    user: Option<UserBody>,
    role: Role,
}

async fn me(State(state): State<AppState>, principal: Principal) -> ApiResult<Json<MeResponse>> {
    match principal {
        Principal::LocalOwner => Ok(Json(MeResponse {
            mode: Mode::Local.as_str(),
            user: None,
            role: Role::Admin,
        })),
        Principal::User(signed_in) => {
            let id = signed_in.id;
            let user = super::with_conn(&state, move |conn| users::get_user(conn, id))
                .await?
                .ok_or_else(ApiError::unauthorized_shared)?;
            Ok(Json(MeResponse {
                mode: Mode::Shared.as_str(),
                role: user.role,
                user: Some(UserBody::from(&user)),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_is_at_most_one_lockout_and_never_zero() {
        assert_eq!(retry_after_ms(Duration::from_secs(90)), 90_000);
        assert_eq!(
            retry_after_ms(Duration::from_secs(3 * 60 * 60)),
            LOCKOUT.as_millis()
        );
        assert_eq!(retry_after_ms(Duration::ZERO), 1);
    }

    #[test]
    fn a_full_table_is_reported_at_most_once_a_minute() {
        let last = Mutex::new(None);
        let t0 = Instant::now();
        assert!(full_notice_due(&last, t0));
        assert!(!full_notice_due(&last, t0 + Duration::from_secs(59)));
        assert!(full_notice_due(&last, t0 + Duration::from_secs(60)));
        assert!(!full_notice_due(&last, t0 + Duration::from_secs(61)));
    }
}
