//! `POST /api/setup` — creating a set of books from the browser or the desktop
//! shell, the same four answers the terminal's onboarding collects.
//!
//! Setup runs once. The guard is here rather than in the client: a second call
//! is a conflict, so no client bug can walk over books that already exist. It
//! needs no exemption from the locked guard — an uninitialized database cannot
//! be locked — and in web mode the session guard applies as it does everywhere
//! else, since the user arrived through the token URL.

use std::path::{Path, PathBuf};

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::db;
use crate::setup::SetupPlan;

use super::super::error::{ApiError, ApiResult};
use super::super::secret::Secret;
use super::super::state::AppState;
use super::status::{current_status, initialized, StatusResponse};

pub fn routes() -> Router<AppState> {
    Router::new().route("/setup", post(post_setup))
}

/// What to do once the books exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SetupAction {
    Fresh,
    Demo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SetupRequest {
    user_name: String,
    company_name: String,
    profile: String,
    #[serde(default)]
    password: Option<Secret>,
    action: SetupAction,
}

async fn post_setup(
    State(state): State<AppState>,
    body: Result<Json<SetupRequest>, JsonRejection>,
) -> ApiResult<Json<StatusResponse>> {
    // `ApiJson` would be the house extractor here, but its rejection carries
    // serde's own text, which quotes the offending value — and one of the
    // values in this body is a password. `POST /api/unlock` declines it for
    // the same reason.
    let Json(request) = body.map_err(|_| {
        ApiError::bad_request(
            "Expected a JSON body of the form {\"userName\": \"...\", \"companyName\": \"...\", \"profile\": \"business\", \"action\": \"fresh\"}.",
        )
    })?;

    let Some(profile) = db::Profile::parse(request.profile.trim()) else {
        return Err(ApiError::bad_request(format!(
            "Unknown profile '{}'. Expected 'business' or 'personal'.",
            request.profile.trim()
        )));
    };

    // Trimmed as the terminal's prompt trims, so a password set here can
    // always be typed back in. Control characters cannot survive the
    // `PRAGMA key = '…'` every later open applies, and would lock the owner
    // out permanently.
    let password = match request
        .password
        .as_ref()
        .map(|secret| secret.expose().trim())
    {
        None | Some("") => None,
        Some(value) if value.chars().any(char::is_control) => {
            return Err(ApiError::bad_request(
                "The password cannot contain control characters.",
            ))
        }
        Some(value) => Some(value.to_string()),
    };

    let action = request.action;

    {
        // The write side, as the data-directory switch takes it: this creates
        // a database file and then rebinds the path every later request reads.
        let _gate = state.db_gate.write().await;

        // One read of the served path, feeding both the guard and the write.
        // Two reads could straddle a settings.json rewritten in between, and
        // the route would check one directory for existing books and create
        // them in another.
        let served = state.db_path();
        if initialized(&served) {
            return Err(ApiError::conflict(
                "These books are already set up.",
                serde_json::json!({ "reason": "already_initialized" }),
            ));
        }

        let plan = SetupPlan {
            user_name: request.user_name.trim().to_string(),
            company_name: request.company_name.trim().to_string(),
            profile,
            password,
            data_dir: served
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        };

        let db_path = tokio::task::spawn_blocking(move || -> ApiResult<PathBuf> {
            let fresh = crate::setup::run(&plan)?;
            match action {
                SetupAction::Fresh => Ok(fresh),
                // Its own directory and its own database, so the demo never
                // sits on top of the books the user is about to keep.
                SetupAction::Demo => Ok(crate::demo::setup_demo_dir()?),
            }
        })
        .await
        .map_err(ApiError::internal)??;

        state.set_db_path(db_path);
    }

    Ok(Json(current_status(&state).await?))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;
    use serde_json::json;

    /// A data directory with no database in it, with settings.json redirected
    /// to a temporary config dir and pointed at that directory.
    fn empty_books() -> (TempConfig, tempfile::TempDir, std::path::PathBuf) {
        crate::db::set_db_password(None);
        let config = TempConfig::new();
        let dir = tempfile::tempdir().expect("tempdir");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = dir.path().to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("save settings");
        let db_path = dir.path().join("nigel.db");
        (config, dir, db_path)
    }

    fn fresh_body() -> serde_json::Value {
        json!({
            "userName": "Marta",
            "companyName": "Cedar Systems",
            "profile": "business",
            "action": "fresh"
        })
    }

    #[tokio::test]
    async fn a_fresh_setup_answers_ready_status() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(&app, "/api/setup", &token, &fresh_body()).await;

        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["initialized"], true);
        assert_eq!(body["locked"], false);
        assert_eq!(body["encrypted"], false);
        assert_eq!(body["companyName"], "Cedar Systems");
        assert_eq!(body["profile"], "business");
        assert_eq!(crate::settings::load_settings().user_name, "Marta");
    }

    #[tokio::test]
    async fn a_personal_setup_keeps_the_personal_chart() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["profile"] = json!("personal");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["profile"], "personal");
    }

    #[tokio::test]
    async fn a_demo_setup_rebinds_to_the_demo_books() {
        let (_config, dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["action"] = json!("demo");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["companyName"], "Acme Consulting LLC");
        let demo_dir = std::fs::canonicalize(dir.path().join("demo")).expect("canonicalize");
        assert_eq!(
            std::fs::canonicalize(answer["dataDir"].as_str().expect("dataDir"))
                .expect("canonicalize"),
            demo_dir,
            "the server is still serving the empty books"
        );

        // The rebind is what makes the next read land on the demo: a rewritten
        // settings.json alone would leave every request on the old path.
        let accounts = ok_json(&app, "/api/accounts", &token).await;
        let names: Vec<&str> = accounts
            .as_array()
            .expect("array")
            .iter()
            .map(|a| a["name"].as_str().expect("name"))
            .collect();
        assert!(
            names.contains(&"BofA Checking"),
            "demo account missing: {names:?}"
        );
    }

    #[tokio::test]
    async fn a_password_leaves_the_database_encrypted_and_unlocked() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!("correct horse battery staple");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["encrypted"], true);
        assert_eq!(
            answer["locked"], false,
            "setup locked the user straight back out"
        );
        assert!(crate::db::is_encrypted(&db_path).expect("probe"));
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn a_password_never_appears_in_the_answer() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!("correct horse battery staple");

        let (_status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert!(!answer.to_string().contains("correct horse"), "{answer}");
        crate::db::set_db_password(None);
    }

    #[tokio::test]
    async fn a_malformed_password_is_refused_without_quoting_it() {
        // serde's type-mismatch text quotes the offending value, and here that
        // value is the password. The rejection is answered with one fixed
        // sentence for the same reason `POST /api/unlock` is.
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!(12345);

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
        assert!(
            !answer.to_string().contains("12345"),
            "the rejection quoted the password back: {answer}"
        );
    }

    #[tokio::test]
    async fn the_books_land_where_the_server_is_looking() {
        // The 409 guard reads the served path; the write has to use that same
        // value, or a settings.json repointed between the two would have the
        // route check one directory and create books in another.
        let (_config, dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);

        let elsewhere = dir.path().join("elsewhere");
        let mut settings = crate::settings::load_settings();
        settings.data_dir = elsewhere.to_string_lossy().to_string();
        crate::settings::save_settings(&settings).expect("repoint settings");

        let (status, answer) = post_json(&app, "/api/setup", &token, &fresh_body()).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert!(
            db_path.exists(),
            "no books at the served path {}",
            db_path.display()
        );
        assert!(
            !elsewhere.join("nigel.db").exists(),
            "the books followed settings.json instead of the served path"
        );
    }

    #[tokio::test]
    async fn setting_up_twice_is_a_conflict() {
        // Setup is not re-runnable, and the guard is the route's rather than
        // the client's: a second call must not walk over existing books.
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        post_json(&app, "/api/setup", &token, &fresh_body()).await;

        let (status, body) = post_json(&app, "/api/setup", &token, &fresh_body()).await;

        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "already_initialized");
    }

    #[tokio::test]
    async fn an_unknown_profile_is_refused() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["profile"] = json!("corporate");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
        assert!(!db_path.exists(), "a bad profile still created books");
    }

    #[tokio::test]
    async fn an_unknown_field_is_refused() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["dataDir"] = json!("/somewhere/else");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{answer}");
    }

    #[tokio::test]
    async fn an_empty_password_is_treated_as_no_password() {
        let (_config, _dir, db_path) = empty_books();
        let (app, token) = app_for(&db_path);
        let mut body = fresh_body();
        body["password"] = json!("   ");

        let (status, answer) = post_json(&app, "/api/setup", &token, &body).await;

        assert_eq!(status, StatusCode::OK, "{answer}");
        assert_eq!(answer["encrypted"], false);
    }

    #[tokio::test]
    async fn setup_needs_a_session_in_web_mode() {
        let (_config, _dir, db_path) = empty_books();
        let (app, _token) = app_for(&db_path);

        let (status, _body) = send(
            &app,
            session_request(
                "POST",
                "/api/setup",
                "not-the-token",
                Some(&fresh_body().to_string()),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!db_path.exists(), "an unauthenticated call created books");
    }
}
