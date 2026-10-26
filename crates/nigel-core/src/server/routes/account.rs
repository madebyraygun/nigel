//! The signed-in user's own account: `POST /api/account/password` and
//! two-step setup, confirmation and removal. Shared mode only — the local
//! owner has no account and gets `409 local_mode`.
//!
//! Every wrong current password, and every wrong code when removing two-step,
//! counts against the sign-in limits for the user's account and client IP, so
//! a stolen session cannot become a way to guess either. Only a right code
//! resets the account's count: proving the password alone takes back that
//! attempt's charge and leaves earlier failures standing, or a request that
//! needs only the password would wipe the count of guessed codes.

use std::time::Instant;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::identity::seal::{self, ServerKey};
use crate::identity::totp;
use crate::identity::users::{self, PasswordChangeError, TotpCheck};

use super::super::error::{ApiError, ApiResult};
use super::super::events::{emit, AuthEvent, EventKind};
use super::super::extract::{PeerAddr, SecretJson};
use super::super::principal::{Principal, SessionUser};
use super::super::ratelimit::{account_key, client_ip, Attempt, ClientIp};
use super::super::secret::Secret;
use super::super::state::AppState;
use super::sessions::{report_full, retry_after_ms};

/// The most an account-route body may be.
pub const ACCOUNT_BODY_LIMIT: usize = 8 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/account/password", post(change_password))
        .route("/account/totp/setup", post(totp_setup))
        .route("/account/totp/confirm", post(totp_confirm))
        .route("/account/totp/remove", post(totp_remove))
        .layer(DefaultBodyLimit::max(ACCOUNT_BODY_LIMIT))
}

fn signed_in(principal: Principal) -> ApiResult<SessionUser> {
    match principal {
        Principal::User(user) => Ok(user),
        Principal::LocalOwner => Err(ApiError::local_mode()),
    }
}

fn event<'a>(kind: EventKind, user: &'a SessionUser, ip: &'a ClientIp) -> AuthEvent<'a> {
    AuthEvent::new(kind)
        .email(&user.email)
        .user(user.id)
        .ip(&ip.log)
        .session(user.session_id)
}

/// Admit an attempt for the signed-in user under the sign-in limits, or
/// refuse it while their account or IP is locked.
fn admit(state: &AppState, user: &SessionUser, ip: &ClientIp) -> ApiResult<Attempt> {
    let attempt = state
        .limiter
        .begin(&account_key(&user.email), true, &ip.bucket, Instant::now())
        .map_err(|wait| ApiError::too_many_wrong_passwords(retry_after_ms(wait)))?;
    report_full(&attempt);
    Ok(attempt)
}

/// Let the attempt's charge stand, and log any lockout it started.
fn charge(state: &AppState, user: &SessionUser, ip: &ClientIp, attempt: Attempt) {
    for bucket in state.limiter.failed(attempt) {
        emit(&event(EventKind::Lockout, user, ip).reason(bucket.as_str()));
    }
}

/// Admit an attempt and check the signed-in user's current password. On a
/// right password the attempt comes back unsettled, for the caller to settle
/// once it knows how the request ends; otherwise it is settled here.
async fn admit_with_password(
    state: &AppState,
    user: &SessionUser,
    ip: &ClientIp,
    password: Secret,
) -> ApiResult<Attempt> {
    let attempt = admit(state, user, ip)?;
    let Some(permit) = state.try_password_check() else {
        state.limiter.not_a_failure(attempt);
        return Err(ApiError::busy());
    };
    let id = user.id;
    let verified = super::with_conn(state, move |conn| {
        let _permit = permit;
        users::verify_user_password(conn, id, password.expose())
    })
    .await;
    match verified {
        Ok(true) => Ok(attempt),
        Ok(false) => {
            charge(state, user, ip, attempt);
            Err(ApiError::wrong_password())
        }
        Err(e) => {
            state.limiter.not_a_failure(attempt);
            Err(e)
        }
    }
}

/// Log why the server key could not be had, for the operator, and answer a
/// `500` that carries neither the path nor the cause.
fn key_unavailable(e: crate::error::NigelError) -> ApiError {
    eprintln!("notice: two-step verification is unavailable: {e}");
    ApiError::totp_key_unavailable()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PasswordChange {
    current_password: Secret,
    new_password: Secret,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PasswordChanged {
    changed: bool,
}

/// Clears `must_change_password` and ends every other session of this user.
async fn change_password(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<PasswordChange>, ApiError>,
) -> ApiResult<Json<PasswordChanged>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    let attempt = admit(&state, &user, &ip)?;
    let Some(permit) = state.try_password_check() else {
        state.limiter.not_a_failure(attempt);
        return Err(ApiError::busy());
    };

    let (id, session_id, now) = (user.id, user.session_id, state.now());
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let ran = super::spawn_blocking_holding(gate, move || {
        let _permit = permit;
        let conn = crate::db::get_connection(&db_path)?;
        users::change_password(
            &conn,
            id,
            request.current_password.expose(),
            request.new_password.expose(),
            Some(session_id),
            now,
        )
    })
    .await;
    let result = match ran {
        Ok((_gate, result)) => result,
        Err(e) => {
            state.limiter.not_a_failure(attempt);
            return Err(e);
        }
    };

    match result {
        Ok(()) => {
            state.limiter.not_a_failure(attempt);
            emit(&event(EventKind::PasswordChanged, &user, &ip));
            Ok(Json(PasswordChanged { changed: true }))
        }
        Err(PasswordChangeError::WrongCurrentPassword) => {
            charge(&state, &user, &ip, attempt);
            Err(ApiError::wrong_password())
        }
        Err(PasswordChangeError::Rule(rule)) => {
            state.limiter.not_a_failure(attempt);
            Err(ApiError::bad_request(rule.to_string())
                .with_details(serde_json::json!({ "reason": rule.reason_code() })))
        }
        Err(PasswordChangeError::Failed(e)) => {
            state.limiter.not_a_failure(attempt);
            Err(e.into())
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotpSetupRequest {
    current_password: Secret,
}

/// The new secret three ways. Its fields are wiped when it drops; the
/// serialized body axum writes from it is not.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TotpSetupResponse {
    setup_id: String,
    secret: String,
    otpauth_uri: String,
    qr_svg: String,
}

impl Drop for TotpSetupResponse {
    fn drop(&mut self) {
        self.secret.zeroize();
        self.otpauth_uri.zeroize();
        self.qr_svg.zeroize();
    }
}

fn qr_svg(uri: &str) -> ApiResult<String> {
    qrcode::QrCode::new(uri.as_bytes())
        .map(|code| {
            code.render::<qrcode::render::svg::Color>()
                .min_dimensions(200, 200)
                .build()
        })
        .map_err(|_| ApiError::internal("Could not draw the QR code."))
}

/// Starts enrollment: a new secret held in memory for ten minutes, returned
/// as base32, an `otpauth://` URI and a QR code. Nothing is stored yet.
async fn totp_setup(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<TotpSetupRequest>, ApiError>,
) -> ApiResult<Json<TotpSetupResponse>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    let attempt = admit_with_password(&state, &user, &ip, request.current_password).await?;
    state.limiter.not_a_failure(attempt);

    let id = user.id;
    let current = super::with_conn(&state, move |conn| users::get_user(conn, id))
        .await?
        .ok_or_else(ApiError::unauthorized_shared)?;
    if current.totp_enabled {
        return Err(totp_already_enabled());
    }

    let secret = totp::generate_secret();
    let uri = totp::otpauth_uri(&current.email, secret.as_ref());
    let response = TotpSetupResponse {
        secret: totp::secret_base32(secret.as_ref()).to_string(),
        otpauth_uri: uri.to_string(),
        qr_svg: qr_svg(&uri)?,
        setup_id: state
            .totp_setups
            .start(user.session_id, user.id, secret, Instant::now()),
    };
    Ok(Json(response))
}

fn totp_already_enabled() -> ApiError {
    ApiError::conflict(
        "Two-step verification is already on. Turn it off before setting it up again.",
        serde_json::json!({ "reason": "totp_enabled" }),
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotpConfirmRequest {
    setup_id: String,
    code: Secret,
}

impl std::fmt::Debug for TotpConfirmRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TotpConfirmRequest(<redacted>)")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TotpChanged {
    totp_enabled: bool,
}

/// Verifies a code against the pending secret, then seals and stores it.
/// The first enrollment on a machine creates `server.key`.
async fn totp_confirm(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<TotpConfirmRequest>, ApiError>,
) -> ApiResult<Json<TotpChanged>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let expired = || {
        ApiError::conflict(
            "That setup has expired. Start again.",
            serde_json::json!({ "reason": "setup_expired" }),
        )
    };
    let (owner, secret) = state
        .totp_setups
        .secret_for(user.session_id, &request.setup_id, Instant::now())
        .ok_or_else(expired)?;
    if owner != user.id {
        return Err(expired());
    }
    let now = state.now();
    let step = totp::matching_step(secret.as_ref(), request.code.expose(), now)
        .ok_or_else(ApiError::code_mismatch)?;

    let id = user.id;
    super::with_conn_api(&state, move |conn| {
        let current = users::get_user(conn, id)?.ok_or_else(ApiError::unauthorized_shared)?;
        if current.totp_enabled {
            return Err(totp_already_enabled());
        }
        let key = ServerKey::load_or_create(&seal::key_path()).map_err(key_unavailable)?;
        users::set_totp_secret(conn, id, &key, secret.as_ref(), step, now)?;
        Ok(())
    })
    .await?;
    state.totp_setups.finish(user.session_id);
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    emit(&event(EventKind::TotpEnabled, &user, &ip));
    Ok(Json(TotpChanged { totp_enabled: true }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotpRemoveRequest {
    current_password: Secret,
    code: Secret,
}

/// Clears the second factor, given the password and a current code.
async fn totp_remove(
    State(state): State<AppState>,
    principal: Principal,
    PeerAddr(peer): PeerAddr,
    headers: HeaderMap,
    body: Result<SecretJson<TotpRemoveRequest>, ApiError>,
) -> ApiResult<Json<TotpChanged>> {
    let user = signed_in(principal)?;
    let SecretJson(request) = body?;
    let ip = client_ip(peer, &headers, state.config.client_ip_header);
    let attempt = admit_with_password(&state, &user, &ip, request.current_password).await?;

    let (id, now, code) = (user.id, state.now(), request.code);
    let checked = super::with_conn_api(&state, move |conn| {
        let Some(key) = ServerKey::load(&seal::key_path()).map_err(key_unavailable)? else {
            return Ok(TotpCheck::Unopenable);
        };
        let check = users::verify_totp(conn, id, &key, code.expose(), now)?;
        if check == TotpCheck::Accepted {
            users::clear_totp(conn, id, now)?;
        }
        Ok(check)
    })
    .await;
    let check = match checked {
        Ok(check) => check,
        Err(e) => {
            state.limiter.not_a_failure(attempt);
            return Err(e);
        }
    };

    match check {
        TotpCheck::Accepted => {
            state.limiter.succeeded(attempt);
            emit(&event(EventKind::TotpRemoved, &user, &ip));
            Ok(Json(TotpChanged {
                totp_enabled: false,
            }))
        }
        TotpCheck::Rejected => {
            charge(&state, &user, &ip, attempt);
            Err(ApiError::code_mismatch())
        }
        TotpCheck::NotEnrolled => {
            state.limiter.not_a_failure(attempt);
            Err(ApiError::conflict(
                "Two-step verification is not on.",
                serde_json::json!({ "reason": "totp_not_enabled" }),
            ))
        }
        TotpCheck::Unopenable => {
            state.limiter.not_a_failure(attempt);
            emit(&event(EventKind::TotpUnavailable, &user, &ip));
            Err(ApiError::totp_unavailable())
        }
    }
}
