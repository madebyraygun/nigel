//! API route modules — one file per domain as endpoints land.
//!
//! Routes fall into two groups. `/ping`, `/status`, and `/unlock` answer while
//! the database is still encrypted and locked; everything else needs the key.
//! The locked guard is layered over the whole `/api` router and exempts that
//! short list by path, so a new endpoint is guarded by default — forgetting to
//! mount it in the right place cannot expose a locked database.

pub mod accounts;
pub mod categories;
pub mod clients;
pub mod documents;
pub mod exports;
pub mod imports;
pub mod invoices;
pub mod reconcile;
pub mod reports;
pub mod review;
pub mod rules;
pub mod settings;
pub mod setup;
pub mod status;
pub mod transactions;

use axum::http::Uri;
use axum::routing::get;
use axum::{middleware, Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Deserializer, Serialize};

use super::error::{ApiError, ApiResult};
use super::state::AppState;

pub fn api_router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/ping", get(ping))
        .merge(status::routes())
        .merge(data_router())
        .fallback(api_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            status::locked_guard,
        ))
}

/// Every route that touches the database — one merge per domain as they land.
fn data_router() -> Router<AppState> {
    let router = Router::new()
        .merge(reports::routes())
        .merge(exports::routes())
        .merge(accounts::routes())
        .merge(categories::routes())
        .merge(rules::routes())
        .merge(imports::routes())
        .merge(transactions::routes())
        .merge(review::routes())
        .merge(reconcile::routes())
        .merge(settings::routes())
        .merge(setup::routes())
        .merge(clients::routes())
        .merge(invoices::routes())
        .merge(documents::routes());

    // A route that exists only to prove `api_router` — the assembly every
    // endpoint is mounted into — actually applies the guard, without pinning
    // that proof to whichever real endpoint happens to exist.
    #[cfg(test)]
    let router = router.route(
        "/_guarded_probe",
        get(|| async { Json(GuardProbe { ok: true }) }),
    );

    router
}

#[cfg(test)]
#[derive(Serialize)]
struct GuardProbe {
    ok: bool,
}

/// Run a database read on the blocking pool. rusqlite is synchronous and a
/// `Connection` is not `Send`-shared, so each request opens its own — WAL and
/// `busy_timeout` handle the concurrency, and there is no pool to size.
///
/// `db::get_connection` reads the process-global password itself, so an
/// unlocked database needs no plumbing through here.
///
/// The `db_gate` read guard is held for the life of the connection. Encrypting,
/// decrypting and switching data directories take the write side, so none of
/// them can rewrite the file underneath a live connection. **A handler that
/// opens a connection without coming through here must take an owned guard
/// (`state.db_gate.clone().read_owned().await`), read `state.db_path()` after
/// it rather than before, and run its blocking work through
/// [`spawn_blocking_holding`]** — `routes::imports`, `routes::status`,
/// `routes::settings` and `routes::setup` all do.
pub(super) async fn with_conn<T, F>(state: &AppState, work: F) -> ApiResult<T>
where
    F: FnOnce(&Connection) -> crate::error::Result<T> + Send + 'static,
    T: Send + 'static,
{
    with_conn_api(state, move |conn| work(conn).map_err(ApiError::from)).await
}

/// [`with_conn`] for work that already speaks in API errors — an export asked
/// for a format this build cannot render owes the caller a `501`, which is a
/// distinction `NigelError` has no way to carry.
pub(super) async fn with_conn_api<T, F>(state: &AppState, work: F) -> ApiResult<T>
where
    F: FnOnce(&Connection) -> ApiResult<T> + Send + 'static,
    T: Send + 'static,
{
    // The guard comes first: the path is read under it, because a data-directory
    // switch holds the write side and a path captured before the wait belongs to
    // the database this request is no longer serving.
    let gate = state.db_gate.clone().read_owned().await;
    let db_path = state.db_path();
    let (_gate, result) = spawn_blocking_holding(gate, move || -> ApiResult<T> {
        let conn = crate::db::get_connection(&db_path)?;
        work(&conn)
    })
    .await?;
    result
}

/// Run `work` on the blocking pool with a `db_gate` guard moved into the task,
/// and hand the guard back when it is done.
///
/// axum drops the handler future of a request whose client disconnected, but a
/// `spawn_blocking` task cannot be cancelled and runs to completion. A guard
/// held by the future would be released at the disconnect while the task still
/// had the database open, letting an encrypt or a data-directory switch rewrite
/// the file underneath it. Owned by the task, the guard is released only after
/// `work` has returned and dropped its connection. A caller with more to do
/// under the guard keeps the one handed back.
pub(super) async fn spawn_blocking_holding<G, T, F>(guard: G, work: F) -> ApiResult<(G, T)>
where
    G: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let result = work();
        (guard, result)
    })
    .await
    .map_err(ApiError::internal)
}

/// Filtering by an account name that does not exist is a wrong question, not an
/// empty answer: `get_register` and `list_reconciliations` would both report a
/// typo as "nothing here".
pub(super) fn ensure_account_exists(conn: &Connection, name: &str) -> crate::error::Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(crate::error::NigelError::UnknownAccount(name.to_string()))
    }
}

/// Re-answer a data-layer `NotFound` with the name of the thing this route was
/// looking up.
///
/// The data layer says only that something was not found, which is right for a
/// terminal and not enough for a handler that resolves both an invoice and its
/// client: a client branching on the status alone has to guess which one is
/// missing. Everything else passes through untouched.
pub(super) fn not_found_because(err: crate::error::NigelError, reason: &str) -> ApiError {
    match err {
        crate::error::NigelError::NotFound(message) => ApiError::not_found_because(message, reason),
        other => ApiError::from(other),
    }
}

/// What a delete answers with. A body rather than a bare `204` so a client can
/// decode every response the same way.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Deleted {
    id: i64,
    deleted: bool,
}

impl Deleted {
    pub(crate) fn new(id: i64) -> Json<Self> {
        Json(Self { id, deleted: true })
    }
}

/// Tell "field absent" from "field explicitly null" in a PATCH body.
///
/// A plain `Option<T>` collapses the two, which is exactly the distinction a
/// partial update needs: absent means leave it alone, `null` means clear it.
/// Used with `#[serde(default, deserialize_with = "double_option")]`.
pub(crate) fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer).map(Some)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PingResponse {
    ok: bool,
    version: &'static str,
}

async fn ping() -> Json<PingResponse> {
    Json(PingResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn api_not_found(uri: Uri) -> ApiError {
    // Nesting strips the mount point, so put it back for a message that matches
    // what the caller actually requested.
    ApiError::not_found(format!("No API endpoint at /api{}", uri.path()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::state::AppState;
    use crate::server::testutil::temp_db;
    use std::sync::mpsc;
    use std::time::Duration;

    /// A client that disconnects drops the handler future while its blocking
    /// work is still running. The gate stays shut until that work has finished
    /// and dropped its connection, so an encrypt or a data-directory switch
    /// waiting on the write side cannot rewrite the file underneath it.
    #[tokio::test]
    async fn a_dropped_request_keeps_the_gate_until_its_blocking_work_ends() {
        let (_dir, db_path) = temp_db();
        let state = AppState::new(db_path, "token".into());
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (release_tx, release_rx) = mpsc::channel::<()>();

        let mut request = Box::pin(with_conn_api(&state, move |conn| {
            conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                .map_err(ApiError::internal)?;
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(())
        }));
        let started = tokio::task::spawn_blocking(move || started_rx.recv());
        tokio::select! {
            _ = &mut request => panic!("the work finished before it was released"),
            started = started => started.unwrap().unwrap(),
        }

        drop(request);
        assert!(
            state.db_gate.try_write().is_err(),
            "the gate opened while the connection was still in use"
        );

        release_tx.send(()).unwrap();
        let _write = tokio::time::timeout(Duration::from_secs(10), state.db_gate.write())
            .await
            .expect("the gate opens once the blocking work has ended");
    }
}
