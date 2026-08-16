//! Invoicing clients: the list, one client in full, and create/edit/delete.
//!
//! The detail route is `client_summary` with serde on it — the same round trip
//! `nigel client show` makes — so the browser and the terminal print one client
//! from one query.
//!
//! The writes are the CLI's own data layer called directly: `add_client` and
//! `update_client` validate the name and refuse a duplicate themselves, and
//! `delete_client` owns the has-invoices guardrail, so this module shapes
//! requests and narrows 404s and does no rule-keeping of its own.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::invoicing::clients::{
    self, ClientContact, ClientInvoiceRow, ClientScope, ClientUpdate, NewContact,
};
use crate::models::Client;

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::{double_option, not_found_because, with_conn, with_conn_api, Deleted};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/clients", get(list).post(create))
        .route("/clients/{id}", get(detail).patch(update).delete(remove))
        // A state transition with a timestamp the server writes, so it gets its
        // own verb rather than a `ClientPatch` field — `POST …/void`'s
        // precedent.
        .route("/clients/{id}/archive", post(archive))
        .route("/clients/{id}/unarchive", post(unarchive))
}

/// A client's own fields flattened alongside its history, rather than nested
/// under a `client` key: a screen showing one client wants one object, and
/// `ClientSummary`'s shape exists for the CLI's benefit.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientDetail {
    #[serde(flatten)]
    client: Client,
    /// Every address, billing first. On the detail and not on the list row,
    /// which stays one query and one cheap payload.
    contacts: Vec<ClientContact>,
    invoices: Vec<ClientInvoiceRow>,
    outstanding: f64,
}

/// Taken as a string so an unrecognised value lands in the error envelope
/// instead of axum's plain-text `Query` rejection — `invoices.rs`'s reasoning.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    include_archived: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<Client>>> {
    let scope = match query.include_archived.as_deref() {
        None | Some("false") => ClientScope::Active,
        Some("true") => ClientScope::All,
        Some(value) => {
            return Err(ApiError::bad_request(format!(
                "Invalid `includeArchived`: expected true or false, got \"{value}\"."
            )))
        }
    };
    Ok(Json(
        with_conn(&state, move |conn| clients::list_clients(conn, scope)).await?,
    ))
}

async fn archive(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Client>> {
    let today = crate::clock::today();
    let client = with_conn_api(&state, move |conn| {
        clients::archive_client(conn, id, &today)
            .map_err(|e| not_found_because(e, "client_not_found"))?;
        Ok(clients::get_client(conn, id)?)
    })
    .await?;
    Ok(Json(client))
}

async fn unarchive(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Client>> {
    let client = with_conn_api(&state, move |conn| {
        clients::unarchive_client(conn, id)
            .map_err(|e| not_found_because(e, "client_not_found"))?;
        Ok(clients::get_client(conn, id)?)
    })
    .await?;
    Ok(Json(client))
}

async fn detail(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<ClientDetail>> {
    let summary = with_conn_api(&state, move |conn| {
        clients::client_summary(conn, id).map_err(|e| not_found_because(e, "client_not_found"))
    })
    .await?;
    Ok(Json(ClientDetail {
        client: summary.client,
        contacts: summary.contacts,
        invoices: summary.invoices,
        outstanding: summary.outstanding,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewClientRequest {
    name: String,
    email: Option<String>,
    billing_address: Option<String>,
    notes: Option<String>,
    /// A plain `Option`, not `double_option`: an empty array is how a caller
    /// clears the list, so `null` would need a second meaning for nothing.
    contacts: Option<Vec<NewContact>>,
}

/// `email` sets the billing address alone; `contacts` replaces the whole list.
/// Sending both would make the order they were applied in visible, so it is a
/// 400 naming them — the CLI's `conflicts_with`, at the wire level.
///
/// **Presence**, not value: `{"email": null, "contacts": [...]}` is still both
/// fields, and `email: null` is a write — it clears the billing address.
fn refuse_email_and_contacts(
    email_present: bool,
    contacts: &Option<Vec<NewContact>>,
) -> ApiResult<()> {
    if email_present && contacts.is_some() {
        return Err(ApiError::bad_request(
            "`email` and `contacts` cannot both be sent: `email` sets the billing address, `contacts` replaces the whole list.",
        ));
    }
    Ok(())
}

async fn create(
    State(state): State<AppState>,
    ApiJson(new): ApiJson<NewClientRequest>,
) -> ApiResult<(StatusCode, Json<Client>)> {
    refuse_email_and_contacts(new.email.is_some(), &new.contacts)?;

    let client = with_conn(&state, move |conn| {
        // One transaction over the row and its addresses: a refused contact
        // list must leave no client behind.
        let tx = conn.unchecked_transaction()?;
        let id = clients::add_client_within(
            &tx,
            &new.name,
            new.billing_address.as_deref(),
            new.notes.as_deref(),
        )?;
        match &new.contacts {
            Some(contacts) => clients::set_contacts_within(&tx, id, contacts)?,
            None => clients::set_billing_email(&tx, id, new.email.as_deref())?,
        }
        tx.commit()?;
        clients::get_client(conn, id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(client)))
}

/// The three nullable columns are `double_option`: absent leaves them alone,
/// `null` clears them. `name` is `NOT NULL`, so it can be renamed and never
/// cleared — which is exactly what a plain `Option` expresses.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientPatch {
    name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    billing_address: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    notes: Option<Option<String>>,
    /// Absent leaves the list alone, present replaces it whole — exactly
    /// `items` on `PATCH /api/invoices/{number}`.
    contacts: Option<Vec<NewContact>>,
}

/// Field for field: the request body *is* the update struct, with no
/// translation layer to keep in step. `contacts` is the one field that is not
/// a client column, so it stays behind.
impl From<&ClientPatch> for ClientUpdate {
    fn from(patch: &ClientPatch) -> Self {
        Self {
            name: patch.name.clone(),
            email: patch.email.clone(),
            billing_address: patch.billing_address.clone(),
            notes: patch.notes.clone(),
        }
    }
}

async fn update(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(patch): ApiJson<ClientPatch>,
) -> ApiResult<Json<Client>> {
    refuse_email_and_contacts(patch.email.is_some(), &patch.contacts)?;

    let update = ClientUpdate::from(&patch);
    // `update_client` refuses an empty update too, as an `Invalid`; naming the
    // fields here is what makes the 400 useful to whoever sent `{}`.
    if update.is_empty() && patch.contacts.is_none() {
        return Err(ApiError::bad_request(
            "Nothing to update — provide at least one of `name`, `email`, `billingAddress`, `notes`, or `contacts`.",
        ));
    }

    let client = with_conn_api(&state, move |conn| {
        // One transaction: a refused contact list leaves the rename unapplied
        // too, so a caller never has to guess which half landed.
        let tx = conn
            .unchecked_transaction()
            .map_err(crate::error::NigelError::from)?;
        if !update.is_empty() {
            clients::update_client_within(&tx, id, &update)
                .map_err(|e| not_found_because(e, "client_not_found"))?;
        }
        if let Some(contacts) = &patch.contacts {
            clients::set_contacts_within(&tx, id, contacts)
                .map_err(|e| not_found_because(e, "client_not_found"))?;
        }
        tx.commit().map_err(crate::error::NigelError::from)?;
        clients::get_client(conn, id).map_err(|e| not_found_because(e, "client_not_found"))
    })
    .await?;
    Ok(Json(client))
}

async fn remove(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<Deleted>> {
    with_conn_api(&state, move |conn| {
        clients::delete_client(conn, id).map_err(|e| not_found_because(e, "client_not_found"))
    })
    .await?;
    Ok(Deleted::new(id))
}

#[cfg(test)]
mod tests {
    use crate::server::testutil::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn clients_list_matches_the_data_layer() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let conn = crate::db::open_connection(&db_path, None).expect("open db");

        let body = ok_json(&app, "/api/clients", &token).await;
        let expected = serde_json::to_value(
            crate::invoicing::clients::list_clients(
                &conn,
                crate::invoicing::clients::ClientScope::Active,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(body, expected);

        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 3);
        // By name, which is the order `list_clients` promises.
        assert_eq!(rows[0]["name"], "Acme Co");
        assert!(rows[0].get("billingAddress").is_some(), "{body}");
        // The client with no email carries an explicit null, not an absent key.
        assert_eq!(rows[1]["name"], "Globex");
        assert!(rows[1]["email"].is_null(), "{body}");
    }

    #[tokio::test]
    async fn a_client_detail_carries_its_contacts() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, patched) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({
                "contacts": [
                    { "email": "dana@acme.test", "name": "Dana Chen" },
                    { "email": "ap@acme.test", "isBilling": true },
                ],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{patched}");

        let body = ok_json(&app, "/api/clients/1", &token).await;
        let contacts = body["contacts"].as_array().expect("contacts");
        assert_eq!(contacts.len(), 2);
        // Billing first, whatever order it arrived in.
        assert_eq!(contacts[0]["email"], "ap@acme.test");
        assert_eq!(contacts[0]["isBilling"], true);
        assert_eq!(contacts[1]["name"], "Dana Chen");
        // And the projection follows it.
        assert_eq!(body["email"], "ap@acme.test");
    }

    #[tokio::test]
    async fn creating_a_client_with_contacts_stores_them_all() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, created) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({
                "name": "Initech",
                "contacts": [
                    { "email": "ap@initech.test", "name": "Ada" },
                    { "email": "dev@initech.test" },
                ],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        assert_eq!(created["email"], "ap@initech.test", "the first is billing");

        let id = created["id"].as_i64().expect("an id");
        let detail = ok_json(&app, &format!("/api/clients/{id}"), &token).await;
        assert_eq!(detail["contacts"].as_array().expect("contacts").len(), 2);
    }

    #[tokio::test]
    async fn patching_contacts_replaces_the_whole_list() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for list in [
            serde_json::json!([{ "email": "a@x.test" }, { "email": "b@x.test" }]),
            serde_json::json!([{ "email": "c@x.test" }]),
        ] {
            let (status, body) = patch_json(
                &app,
                "/api/clients/1",
                &token,
                &serde_json::json!({ "contacts": list }),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }

        let detail = ok_json(&app, "/api/clients/1", &token).await;
        let contacts = detail["contacts"].as_array().expect("contacts");
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0]["email"], "c@x.test");
    }

    /// Atomicity, on create: the contact list is refused after the client row
    /// has been written, and neither survives.
    #[tokio::test]
    async fn a_refused_contact_list_leaves_no_client_row() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({
                "name": "Initech",
                "contacts": [
                    { "email": "a@x.test", "isBilling": true },
                    { "email": "b@x.test", "isBilling": true },
                ],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");

        let names: Vec<String> = ok_json(&app, "/api/clients", &token)
            .await
            .as_array()
            .expect("a bare array")
            .iter()
            .map(|c| c["name"].as_str().expect("name").to_string())
            .collect();
        assert!(
            !names.contains(&"Initech".to_string()),
            "the client row outlived the request that was refused: {names:?}"
        );
    }

    /// Atomicity, on patch: the rename and the contact list are one write.
    #[tokio::test]
    async fn a_refused_contact_list_leaves_the_rename_unapplied() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({
                "name": "Acme Corporation",
                "contacts": [
                    { "email": "a@x.test", "isBilling": true },
                    { "email": "b@x.test", "isBilling": true },
                ],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");

        let detail = ok_json(&app, "/api/clients/1", &token).await;
        assert_eq!(detail["name"], "Acme Co", "half the edit landed");
        assert_eq!(detail["email"], "ap@acme.test");
    }

    /// The guard is about which fields arrived, not what they carried: `null`
    /// is a write — it clears the billing address.
    #[tokio::test]
    async fn a_null_email_beside_contacts_is_still_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({
                "email": null,
                "contacts": [{ "email": "dana@acme.test" }],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        let message = json["error"]["message"].as_str().expect("a message");
        assert!(
            message.contains("email") && message.contains("contacts"),
            "{message}"
        );
    }

    #[tokio::test]
    async fn an_absent_contacts_field_leaves_the_list_alone() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "name": "Acme Corporation" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let detail = ok_json(&app, "/api/clients/1", &token).await;
        assert_eq!(detail["contacts"].as_array().expect("contacts").len(), 1);
        assert_eq!(detail["email"], "ap@acme.test");
    }

    #[tokio::test]
    async fn email_and_contacts_in_one_body_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = serde_json::json!({
            "email": "ap@acme.test",
            "contacts": [{ "email": "dana@acme.test" }],
        });

        let (status, json) = patch_json(&app, "/api/clients/1", &token, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        let message = json["error"]["message"].as_str().expect("a message");
        assert!(
            message.contains("email") && message.contains("contacts"),
            "{message}"
        );

        let (status, json) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({
                "name": "Initech",
                "email": "ap@initech.test",
                "contacts": [{ "email": "dev@initech.test" }],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    }

    #[tokio::test]
    async fn two_billing_contacts_is_a_400_from_the_data_layers_own_check() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({
                "contacts": [
                    { "email": "a@x.test", "isBilling": true },
                    { "email": "b@x.test", "isBilling": true },
                ],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert!(
            json["error"]["message"]
                .as_str()
                .expect("a message")
                .contains("billing recipient"),
            "{json}"
        );

        // Refused before anything was deleted.
        let detail = ok_json(&app, "/api/clients/1", &token).await;
        assert_eq!(detail["email"], "ap@acme.test");
    }

    /// The list stays bare `Client` rows and one query.
    #[tokio::test]
    async fn the_client_list_still_carries_one_email_per_row() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/clients", &token).await;
        for row in body.as_array().expect("a bare array") {
            assert!(row.get("contacts").is_none(), "{row}");
            assert!(row.get("email").is_some(), "{row}");
        }
    }

    #[tokio::test]
    async fn the_client_list_hides_archived_clients_by_default() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/clients", &token).await;
        let names: Vec<&str> = body
            .as_array()
            .expect("a bare array")
            .iter()
            .map(|c| c["name"].as_str().expect("name"))
            .collect();
        assert_eq!(names, vec!["Acme Co", "Globex", "Northwind Traders"]);
    }

    #[tokio::test]
    async fn include_archived_shows_them_with_the_timestamp() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/clients?includeArchived=true", &token).await;
        let rows = body.as_array().expect("a bare array");
        assert_eq!(rows.len(), 4);
        let umbrella = rows.last().expect("the archived client");
        assert_eq!(umbrella["name"], "Umbrella Corp");
        assert_eq!(umbrella["archivedAt"], "2026-03-01");
        // An active row carries an explicit null, not an absent key.
        assert!(rows[0]["archivedAt"].is_null(), "{body}");
    }

    /// `false` is the default spelled out, which is what `docs/api.md` says.
    #[tokio::test]
    async fn include_archived_false_is_the_default_spelled_out() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let explicit = ok_json(&app, "/api/clients?includeArchived=false", &token).await;
        let default = ok_json(&app, "/api/clients", &token).await;
        assert_eq!(explicit, default);
    }

    #[tokio::test]
    async fn an_unrecognised_include_archived_value_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/clients?includeArchived=maybe", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("a message")
                .contains("includeArchived"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn archive_and_unarchive_answer_the_refreshed_client() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, archived) = post_json(
            &app,
            "/api/clients/2/archive",
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{archived}");
        assert_eq!(archived["name"], "Globex");
        assert!(!archived["archivedAt"].is_null(), "{archived}");

        // Gone from the default list, still there with the flag.
        let body = ok_json(&app, "/api/clients", &token).await;
        assert_eq!(body.as_array().expect("array").len(), 2);

        let (status, restored) = post_json(
            &app,
            "/api/clients/2/unarchive",
            &token,
            &serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{restored}");
        assert!(restored["archivedAt"].is_null(), "{restored}");
    }

    #[tokio::test]
    async fn archiving_an_unknown_client_is_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for path in [
            "/api/clients/999999/archive",
            "/api/clients/999999/unarchive",
        ] {
            let (status, body) = post_json(&app, path, &token, &serde_json::json!({})).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
            assert_eq!(body["error"]["details"]["reason"], "client_not_found");
        }
    }

    #[tokio::test]
    async fn an_archived_client_cannot_be_given_a_new_invoice() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/invoices",
            &token,
            &serde_json::json!({
                "clientId": 4,
                "issueDate": "2026-03-20",
                "currency": "USD",
                "items": [{ "description": "Work", "quantity": 1.0, "unitAmount": 100.0 }],
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_archived");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("a message")
                .contains("Umbrella Corp"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn a_client_detail_carries_its_invoices_and_open_balance() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/clients/1", &token).await;
        // Flattened: the client's own fields sit beside the history.
        assert_eq!(body["name"], "Acme Co");
        assert_eq!(body["email"], "ap@acme.test");

        let invoices = body["invoices"].as_array().expect("invoices");
        assert_eq!(invoices.len(), 2);
        // Newest number first.
        assert_eq!(invoices[0]["number"], 1251);
        assert_eq!(invoices[1]["number"], 1250);

        // 1251 open at 1850, 1250 open at 3200 - 2000.
        assert_eq!(body["outstanding"], 3050.0);
    }

    #[tokio::test]
    async fn an_unknown_client_id_is_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/clients/999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["code"], "not_found");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");
    }

    #[tokio::test]
    async fn a_client_can_be_created_edited_and_deleted() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, created) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({
                "name": "Initech",
                "email": "ap@initech.test",
                "billingAddress": "9 Cubicle Way",
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        let id = created["id"].as_i64().expect("an id");
        assert_eq!(created["name"], "Initech");
        assert_eq!(created["billingAddress"], "9 Cubicle Way");
        // An omitted optional field is null, not absent.
        assert!(created["notes"].is_null(), "{created}");

        let (status, edited) = patch_json(
            &app,
            &format!("/api/clients/{id}"),
            &token,
            &serde_json::json!({ "name": "Initech LLC" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{edited}");
        assert_eq!(edited["name"], "Initech LLC");
        assert_eq!(
            edited["email"], "ap@initech.test",
            "untouched by the rename"
        );

        let (status, deleted) = delete_json(&app, &format!("/api/clients/{id}"), &token).await;
        assert_eq!(status, StatusCode::OK, "{deleted}");
        assert_eq!(deleted["deleted"], true);
        assert_eq!(deleted["id"], id);

        let listed = ok_json(&app, "/api/clients", &token).await;
        assert!(
            !listed.as_array().unwrap().iter().any(|c| c["id"] == id),
            "a deleted client is off the list: {listed}"
        );
    }

    #[tokio::test]
    async fn a_duplicate_client_name_is_a_409_with_the_name() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_json(
            &app,
            "/api/clients",
            &token,
            &serde_json::json!({ "name": "Acme Co" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_name");
        assert_eq!(body["error"]["details"]["name"], "Acme Co");

        // Renaming onto a taken name is the same refusal.
        let (status, body) = patch_json(
            &app,
            "/api/clients/2",
            &token,
            &serde_json::json!({ "name": "Acme Co" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_name");
    }

    #[tokio::test]
    async fn a_patch_can_clear_an_email_but_omitting_it_leaves_it() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, kept) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "notes": "pays late" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{kept}");
        assert_eq!(kept["email"], "ap@acme.test", "absent leaves it alone");
        assert_eq!(kept["notes"], "pays late");

        let (status, cleared) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "email": null }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{cleared}");
        assert!(cleared["email"].is_null(), "null clears it: {cleared}");
        assert_eq!(cleared["notes"], "pays late", "and touches nothing else");
    }

    #[tokio::test]
    async fn an_all_absent_client_patch_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) =
            patch_json(&app, "/api/clients/1", &token, &serde_json::json!({})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }

    #[tokio::test]
    async fn deleting_a_client_with_invoices_is_blocked_with_a_count() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        // Globex owns 1247 (void) and 1249 (overdue): a void invoice counts.
        let (status, body) = delete_json(&app, "/api/clients/2", &token).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "has_invoices");
        assert_eq!(body["error"]["details"]["count"], 2);
        assert_eq!(
            body["error"]["message"],
            "Cannot delete: client has 2 invoices"
        );

        // Refused means refused.
        let still_there = ok_json(&app, "/api/clients/2", &token).await;
        assert_eq!(still_there["name"], "Globex");
    }

    #[tokio::test]
    async fn an_empty_client_name_is_a_400() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for body in [
            serde_json::json!({ "name": "   " }),
            serde_json::json!({ "name": "" }),
        ] {
            let (status, json) = post_json(&app, "/api/clients", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {json}");
        }

        let (status, json) = patch_json(
            &app,
            "/api/clients/1",
            &token,
            &serde_json::json!({ "name": " " }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    }

    #[tokio::test]
    async fn editing_or_deleting_an_unknown_client_is_a_404_with_a_reason() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/clients/999999",
            &token,
            &serde_json::json!({ "name": "Ghost" }),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");

        let (status, body) = delete_json(&app, "/api/clients/999999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");
    }

    #[tokio::test]
    async fn a_non_numeric_client_id_answers_in_the_envelope() {
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = get_json(&app, "/api/clients/acme", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }
}
