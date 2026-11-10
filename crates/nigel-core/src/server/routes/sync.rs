//! SimpleFIN bridge sync: claiming a setup token, linking the bridge's
//! accounts to Nigel accounts, and running a sync.
//!
//! Parity target is `cli/sync.rs`. The access URL is a credential and never
//! leaves the server: no response carries it and no error quotes it — the
//! client module reports transport failures without their URL.
//!
//! Bridge calls are blocking `reqwest` and may take as long as the bridge
//! takes to ask the banks, so they run on the blocking pool *without* the
//! `db_gate`: holding it across a two-minute request would stall an encrypt or
//! a data-directory switch for no reason. Database work happens afterwards,
//! under the gate, through [`with_conn`] or [`gated`].

use std::path::PathBuf;

use axum::extract::State;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::backup;
use crate::categorizer::categorize_transactions;
use crate::error::NigelError;
use crate::settings;
use crate::simplefin::{self, AccessUrl, AccountSet, AccountsQuery, LinkedAccount, SyncReport};

use super::super::error::{ApiError, ApiErrorCode, ApiResult};
use super::super::extract::ApiJson;
use super::super::state::AppState;
use super::with_conn;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sync", get(get_sync))
        .route("/sync/setup", post(post_setup))
        .route("/sync/accounts/refresh", post(post_refresh))
        .route("/sync/accounts/map", put(put_map))
        .route("/sync/run", post(post_run))
}

const NOT_CONFIGURED: &str =
    "SimpleFIN is not set up. Paste a setup token from your bridge to connect it.";

const NOTHING_LINKED: &str =
    "None of your bank accounts is linked to a Nigel account yet. Pick one for at least one of them, then sync.";

/// What the sync settings show: whether a bridge is connected, and every
/// account it has reported. Never the access URL.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    configured: bool,
    accounts: Vec<LinkedAccount>,
    /// Messages the bridge sent with this answer, sanitized for display.
    warnings: Vec<String>,
}

fn configured() -> ApiResult<bool> {
    Ok(settings::try_load_settings()?
        .simplefin_access_url
        .is_some())
}

fn stored_access() -> ApiResult<AccessUrl> {
    let raw = settings::try_load_settings()?
        .simplefin_access_url
        .ok_or_else(|| ApiError::bad_request(NOT_CONFIGURED))?;
    Ok(AccessUrl::parse(&raw)?)
}

/// A bridge that could not be reached, or answered in a way Nigel cannot read,
/// is the bridge's problem and says so. A refusal the user can act on — a
/// claimed token, revoked access, an unpaid subscription — stays a `400` with
/// the client module's own sentence.
fn upstream(err: NigelError) -> ApiError {
    match err {
        NigelError::Other(message) => ApiError::new(ApiErrorCode::UpstreamFailed, message)
            .with_details(serde_json::json!({ "service": "simplefin" })),
        other => ApiError::from(other),
    }
}

/// Run a bridge call on the blocking pool, outside the `db_gate`.
async fn off_runtime<T, F>(work: F) -> ApiResult<T>
where
    F: FnOnce() -> crate::error::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(ApiError::internal)?
        .map_err(upstream)
}

/// Blocking work that needs the data directory as well as a connection,
/// under the `db_gate` the way `routes::imports` takes it: the paths are read
/// after the guard opens, and the guard is owned by the blocking task.
async fn gated<T, F>(state: &AppState, work: F) -> ApiResult<T>
where
    F: FnOnce(PathBuf, PathBuf) -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let data_dir = state.data_dir();
    let (_gate, result) =
        super::spawn_blocking_holding(gate, move || work(db_path, data_dir)).await?;
    result
}

fn sanitized(set: &AccountSet) -> Vec<String> {
    set.errors
        .iter()
        .map(|m| simplefin::client::sanitize_message(m))
        .filter(|m| !m.is_empty())
        .collect()
}

/// Remember what the bridge returned (when it returned anything) and answer
/// with the stored list.
async fn state_after(
    state: &AppState,
    set: Option<AccountSet>,
    mut warnings: Vec<String>,
) -> ApiResult<Json<SyncState>> {
    let configured = configured()?;
    if let Some(set) = &set {
        warnings.extend(sanitized(set));
    }
    let accounts = with_conn(state, move |conn| {
        if let Some(set) = &set {
            simplefin::remember_accounts(conn, set)?;
        }
        simplefin::list_accounts(conn)
    })
    .await?;
    Ok(Json(SyncState {
        configured,
        accounts,
        warnings,
    }))
}

fn balances_only() -> AccountsQuery {
    AccountsQuery {
        balances_only: true,
        ..AccountsQuery::default()
    }
}

async fn get_sync(State(state): State<AppState>) -> ApiResult<Json<SyncState>> {
    state_after(&state, None, Vec::new()).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetupRequest {
    token: String,
}

/// Claim a setup token, store the access URL, and read the bridge's accounts.
///
/// A token can be claimed once. If the claim succeeds and the account read
/// that follows does not, the connection is kept and the failure comes back
/// as a warning — answering an error would invite a retry with a token that
/// is already spent.
async fn post_setup(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<SetupRequest>,
) -> ApiResult<Json<SyncState>> {
    let token = request.token.trim().to_string();
    if token.is_empty() {
        return Err(ApiError::bad_request(
            "Paste the setup token from your SimpleFIN bridge.",
        ));
    }
    let access = off_runtime(move || {
        let access = simplefin::claim(&token)?;
        let mut stored = settings::try_load_settings()?;
        stored.simplefin_access_url = Some(access.expose().to_string());
        settings::save_settings(&stored)?;
        Ok(access)
    })
    .await?;

    let mut warnings = Vec::new();
    let set = match off_runtime(move || simplefin::fetch_accounts(&access, &balances_only())).await
    {
        Ok(set) => Some(set),
        Err(err) => {
            warnings.push(format!(
                "Connected, but the bridge's accounts could not be read yet: {}",
                err.message()
            ));
            None
        }
    };
    state_after(&state, set, warnings).await
}

/// Ask the bridge for its accounts and balances (no transactions) and
/// remember any new ones, keeping existing links.
async fn post_refresh(State(state): State<AppState>) -> ApiResult<Json<SyncState>> {
    let access = stored_access()?;
    let set = off_runtime(move || simplefin::fetch_accounts(&access, &balances_only())).await?;
    state_after(&state, Some(set), Vec::new()).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MapRequest {
    key: String,
    /// The Nigel account to sync into, or `null` to stop syncing this one.
    account: Option<String>,
    /// Whether this feed supplies the account's bank-reported balance.
    /// Absent means it does; `false` maps a transactions-only feed.
    #[serde(default)]
    supplies_balance: Option<bool>,
}

async fn put_map(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<MapRequest>,
) -> ApiResult<Json<SyncState>> {
    let configured = configured()?;
    let accounts = with_conn(&state, move |conn| {
        match request.account.as_deref().map(str::trim) {
            Some(account) if !account.is_empty() => simplefin::map_account(
                conn,
                &request.key,
                account,
                request.supplies_balance.unwrap_or(true),
            )?,
            _ => simplefin::unmap_account(conn, &request.key)?,
        }
        simplefin::list_accounts(conn)
    })
    .await?;
    Ok(Json(SyncState {
        configured,
        accounts,
        warnings: Vec::new(),
    }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunRequest {
    #[serde(default)]
    dry_run: bool,
    /// `YYYY-MM-DD`: read every linked account from this day instead of
    /// where each one left off.
    #[serde(default)]
    since: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    #[serde(flatten)]
    report: SyncReport,
    dry_run: bool,
    /// Present when the sync imported something and categorization ran.
    categorized: Option<usize>,
    /// Counts the whole ledger, not just this sync.
    still_flagged: Option<usize>,
    /// Where the pre-sync snapshot went, when one was taken.
    snapshot: Option<String>,
}

/// The CLI's sequence: count first with a dry run; then, for real, snapshot
/// only when something will be imported, and apply and categorize in one
/// transaction.
async fn post_run(
    State(state): State<AppState>,
    ApiJson(request): ApiJson<RunRequest>,
) -> ApiResult<Json<RunResult>> {
    let since = request
        .since
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| ApiError::bad_request(format!("since {d:?} is not a YYYY-MM-DD date")))
        })
        .transpose()?;
    let dry_run = request.dry_run;
    let access = stored_access()?;

    let query = with_conn(&state, move |conn| simplefin::query_for(conn, since))
        .await?
        .ok_or_else(|| {
            ApiError::bad_request(NOTHING_LINKED)
                .with_details(serde_json::json!({ "reason": "nothing_linked" }))
        })?;
    let set = off_runtime(move || simplefin::fetch_accounts(&access, &query)).await?;

    let today = crate::clock::today();
    let result = gated(&state, move |db_path, data_dir| -> ApiResult<RunResult> {
        let conn = crate::db::get_connection(&db_path)?;
        let preview = simplefin::apply(&conn, &set, since, &today, true)?;
        if dry_run {
            return Ok(RunResult {
                report: preview,
                dry_run: true,
                categorized: None,
                still_flagged: None,
                snapshot: None,
            });
        }

        // A sync with nothing new changes only the reported balances and the
        // last-sync day, so a quiet day leaves no snapshot behind.
        let snapshot = if preview.imported() > 0 {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let path = data_dir.join(format!("snapshots/pre-sync-{stamp}.db"));
            backup::snapshot(&conn, &path)?;
            Some(path.display().to_string())
        } else {
            None
        };

        let tx = conn.unchecked_transaction().map_err(NigelError::from)?;
        let report = simplefin::apply(&tx, &set, since, &today, false)?;
        let counts = if report.imported() > 0 {
            Some(categorize_transactions(&tx)?)
        } else {
            None
        };
        tx.commit().map_err(NigelError::from)?;

        Ok(RunResult {
            report,
            dry_run: false,
            categorized: counts.as_ref().map(|c| c.categorized),
            still_flagged: counts.as_ref().map(|c| c.still_flagged),
            snapshot,
        })
    })
    .await?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use chrono::TimeZone;
    use serde_json::json;

    use crate::server::testutil::*;
    use crate::simplefin::testing::{StubBridge, STUB_PASSWORD};

    fn fixture() -> (TempConfig, tempfile::TempDir, std::path::PathBuf) {
        let config = TempConfig::new();
        let (dir, db_path) = temp_db();
        let conn = crate::db::open_connection(&db_path, None).expect("open db");
        for name in ["Operating", "Savings"] {
            conn.execute(
                "INSERT INTO accounts (name, account_type) VALUES (?1, 'checking')",
                [name],
            )
            .expect("account");
        }
        (config, dir, db_path)
    }

    fn noon(day: &str) -> i64 {
        let date = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
        chrono::Local
            .from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
            .unwrap()
            .timestamp()
    }

    fn account(id: &str, txns: &[(&str, &str, &str)]) -> serde_json::Value {
        let transactions: Vec<_> = txns
            .iter()
            .map(|(tid, day, amount)| {
                json!({ "id": tid, "posted": noon(day), "amount": amount, "description": format!("Acme {tid}") })
            })
            .collect();
        json!({
            "org": { "domain": "harborvale.example", "name": "Harbor & Vale Bank" },
            "id": id,
            "name": format!("Checking {id}"),
            "currency": "USD",
            "balance": "1250.00",
            "balance-date": noon("2025-03-10"),
            "transactions": transactions,
        })
    }

    fn bridge_body(accounts: Vec<serde_json::Value>) -> String {
        json!({ "errors": [], "accounts": accounts }).to_string()
    }

    fn store_access(bridge: &StubBridge) {
        let mut stored = crate::settings::try_load_settings().unwrap();
        stored.simplefin_access_url = Some(bridge.access_url_string());
        crate::settings::save_settings(&stored).unwrap();
    }

    fn link(db_path: &std::path::Path, id: &str, account: &str) {
        let conn = crate::db::open_connection(db_path, None).unwrap();
        conn.execute(
            "INSERT INTO simplefin_accounts (sfin_key, org_name, name, currency) \
             VALUES (?1, 'Harbor & Vale Bank', ?2, 'USD')",
            [format!("harborvale.example/{id}"), format!("Checking {id}")],
        )
        .unwrap();
        crate::simplefin::map_account(&conn, &format!("harborvale.example/{id}"), account, true)
            .unwrap();
    }

    fn count(db_path: &std::path::Path) -> i64 {
        let conn = crate::db::open_connection(db_path, None).unwrap();
        conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))
            .unwrap()
    }

    #[tokio::test]
    async fn an_unconfigured_bridge_reports_so_and_refuses_to_sync() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/sync", &token).await;
        assert_eq!(
            body,
            json!({ "configured": false, "accounts": [], "warnings": [] })
        );

        for uri in ["/api/sync/run", "/api/sync/accounts/refresh"] {
            let (status, body) = post_json(&app, uri, &token, &json!({})).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
            assert!(
                body["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("not set up"),
                "{body}"
            );
        }
    }

    #[tokio::test]
    async fn setup_claims_stores_and_lists_without_ever_answering_the_access_url() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let bridge = StubBridge::start(vec![(
            200,
            bridge_body(vec![account("1", &[]), account("2", &[])]),
        )]);
        let claim = StubBridge::start(vec![(200, bridge.access_url_string())]);

        let (status, body) = post_json(
            &app,
            "/api/sync/setup",
            &token,
            &json!({ "token": claim.setup_token() }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["configured"], true);
        assert_eq!(body["accounts"].as_array().unwrap().len(), 2);
        assert_eq!(body["accounts"][0]["key"], "harborvale.example/1");
        assert_eq!(body["accounts"][0]["account"], json!(null));
        assert!(!body.to_string().contains(STUB_PASSWORD), "{body}");
        assert!(bridge.seen()[0].target.contains("balances-only=1"));

        assert_eq!(
            crate::settings::try_load_settings()
                .unwrap()
                .simplefin_access_url
                .as_deref(),
            Some(bridge.access_url_string().as_str())
        );

        let listed = ok_json(&app, "/api/sync", &token).await;
        assert_eq!(listed["configured"], true);
        assert_eq!(listed["accounts"].as_array().unwrap().len(), 2);
        assert!(!listed.to_string().contains(STUB_PASSWORD), "{listed}");
    }

    #[tokio::test]
    async fn a_refused_token_is_a_bad_request_with_the_reason() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let claim = StubBridge::start(vec![(403, String::new())]);

        let (status, body) = post_json(
            &app,
            "/api/sync/setup",
            &token,
            &json!({ "token": claim.setup_token() }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already been claimed"));
        assert!(crate::settings::try_load_settings()
            .unwrap()
            .simplefin_access_url
            .is_none());
    }

    #[tokio::test]
    async fn an_unreachable_bridge_is_an_upstream_failure() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let bridge = StubBridge::start(vec![]);
        store_access(&bridge);
        // The stub has no responses, so its listener is gone.
        std::thread::sleep(std::time::Duration::from_millis(50));

        let (status, body) =
            post_json(&app, "/api/sync/accounts/refresh", &token, &json!({})).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
        assert_eq!(body["error"]["details"]["service"], "simplefin");
        assert!(!body.to_string().contains(STUB_PASSWORD), "{body}");
    }

    #[tokio::test]
    async fn accounts_are_mapped_and_unmapped() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let bridge = StubBridge::start(vec![(200, bridge_body(vec![account("1", &[])]))]);
        store_access(&bridge);
        let (status, _) = post_json(&app, "/api/sync/accounts/refresh", &token, &json!({})).await;
        assert_eq!(status, StatusCode::OK);

        let key = "harborvale.example/1";
        let (status, body) = put_json(
            &app,
            "/api/sync/accounts/map",
            &token,
            &json!({ "key": key, "account": "Operating" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["accounts"][0]["account"], "Operating");

        let (status, body) = put_json(
            &app,
            "/api/sync/accounts/map",
            &token,
            &json!({ "key": key, "account": null }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["accounts"][0]["account"], json!(null));

        let (status, body) = put_json(
            &app,
            "/api/sync/accounts/map",
            &token,
            &json!({ "key": "nowhere/9", "account": "Operating" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

        let (status, body) = put_json(
            &app,
            "/api/sync/accounts/map",
            &token,
            &json!({ "key": key, "account": "No Such Account" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "account_not_found");
    }

    #[tokio::test]
    async fn a_second_balance_feed_is_refused_and_a_transactions_only_feed_is_not() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        link(&db_path, "1", "Operating");
        link(&db_path, "2", "Savings");

        let (status, body) = put_json(
            &app,
            "/api/sync/accounts/map",
            &token,
            &json!({ "key": "harborvale.example/2", "account": "Operating" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let message = body["error"]["message"].as_str().unwrap();
        assert!(message.contains("harborvale.example/1"), "{message}");
        assert!(message.contains("transactions-only"), "{message}");

        let (status, body) = put_json(
            &app,
            "/api/sync/accounts/map",
            &token,
            &json!({
                "key": "harborvale.example/2",
                "account": "Operating",
                "suppliesBalance": false
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let accounts = body["accounts"].as_array().unwrap();
        let feed = |key: &str| {
            accounts
                .iter()
                .find(|a| a["key"] == key)
                .unwrap_or_else(|| panic!("{key} missing from {body}"))
        };
        assert_eq!(feed("harborvale.example/1")["account"], "Operating");
        assert_eq!(feed("harborvale.example/1")["suppliesBalance"], true);
        assert_eq!(feed("harborvale.example/2")["account"], "Operating");
        assert_eq!(feed("harborvale.example/2")["suppliesBalance"], false);
    }

    #[tokio::test]
    async fn nothing_linked_is_a_bad_request() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let bridge = StubBridge::start(vec![]);
        store_access(&bridge);

        let (status, body) = post_json(&app, "/api/sync/run", &token, &json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "nothing_linked");
    }

    #[tokio::test]
    async fn a_dry_run_counts_and_a_real_run_imports_and_categorizes() {
        let (_config, dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        link(&db_path, "1", "Operating");
        let feed = bridge_body(vec![
            account(
                "1",
                &[
                    ("t1", "2025-03-01", "-12.50"),
                    ("t2", "2025-03-02", "300.00"),
                ],
            ),
            account("2", &[]),
        ]);
        let bridge = StubBridge::start(vec![(200, feed.clone()), (200, feed.clone()), (200, feed)]);
        store_access(&bridge);

        let (status, dry) = post_json(
            &app,
            "/api/sync/run",
            &token,
            &json!({ "dryRun": true, "since": "2025-01-01" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{dry}");
        assert_eq!(dry["dryRun"], true);
        assert_eq!(dry["accounts"][0]["imported"], 2);
        assert_eq!(dry["accounts"][0]["balance"]["balance"], 1250.0);
        assert_eq!(dry["unmapped"], json!(["Harbor & Vale Bank — Checking 2"]));
        assert_eq!(dry["categorized"], json!(null));
        assert_eq!(dry["snapshot"], json!(null));
        assert_eq!(count(&db_path), 0);
        assert!(!dir.path().join("snapshots").exists());

        let (status, real) = post_json(&app, "/api/sync/run", &token, &json!({})).await;
        assert_eq!(status, StatusCode::OK, "{real}");
        assert_eq!(real["dryRun"], false);
        assert_eq!(real["accounts"][0]["imported"], 2);
        assert!(real["accounts"][0]["importId"].is_i64());
        assert!(real["categorized"].is_u64());
        assert!(real["stillFlagged"].is_u64());
        assert!(real["snapshot"].as_str().unwrap().contains("pre-sync-"));
        assert_eq!(count(&db_path), 2);

        let (status, again) = post_json(
            &app,
            "/api/sync/run",
            &token,
            &json!({ "since": "2025-01-01" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{again}");
        assert_eq!(again["accounts"][0]["imported"], 0);
        assert_eq!(again["accounts"][0]["skipped"], 2);
        assert_eq!(again["snapshot"], json!(null));
        assert_eq!(again["categorized"], json!(null));
        assert_eq!(count(&db_path), 2);

        let listed = ok_json(&app, "/api/sync", &token).await;
        assert!(listed["accounts"][0]["lastSynced"].is_string());
    }

    #[tokio::test]
    async fn a_user_below_admin_can_neither_set_up_nor_run_a_sync() {
        use crate::identity::sessions::SessionKind;
        use crate::identity::Role;
        use axum::body::Body;
        use axum::http::header;

        let (_config, _dir, db_path) = fixture();
        add_ready_user(&db_path, "pat@example.com", Role::Admin);
        let viewer = add_ready_user(&db_path, "sam@example.com", Role::ReadOnly);
        let token = session_for(
            &db_path,
            viewer.id,
            SessionKind::Bearer,
            crate::clock::now_utc(),
        );
        let (app, _state) = served_app(&db_path, public_config());

        for (method, uri, body) in [
            ("GET", "/api/sync", ""),
            ("POST", "/api/sync/setup", r#"{"token":"x"}"#),
            ("POST", "/api/sync/accounts/refresh", "{}"),
            (
                "PUT",
                "/api/sync/accounts/map",
                r#"{"key":"x","account":null}"#,
            ),
            ("POST", "/api/sync/run", r#"{"dryRun":true}"#),
        ] {
            let request = public_request(method, uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap();
            let (status, json) = send(&app, request).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}: {json}");
            assert_eq!(json["error"]["details"]["reason"], "role", "{uri}");
        }
        assert!(crate::settings::try_load_settings()
            .unwrap()
            .simplefin_access_url
            .is_none());
    }

    #[tokio::test]
    async fn a_bad_since_is_a_bad_request() {
        let (_config, _dir, db_path) = fixture();
        let (app, token) = app_for(&db_path);
        let (status, body) = post_json(
            &app,
            "/api/sync/run",
            &token,
            &json!({ "since": "March 1st" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
}
