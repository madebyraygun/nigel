//! Documents: the list, one document in full, the active kinds, and the
//! sandboxed preview of the page a signer would receive.
//!
//! Every `can*` flag the detail response reports is `guards::can` — the table
//! the data layer enforces — plus the archived-client check for the two actions
//! whose data-layer functions make it, so a flag never disagrees with the 409
//! it predicts.
//!
//! No token crosses the wire as a field. A sent version's recipients carry the
//! computed `pageUrl`, which is the one place a recipient's address appears.

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::documents::guards::{can, ensure_client_active_for_documents, Action};
use crate::documents::kinds::list_kinds;
use crate::documents::model::{
    ChangeRequest, Document, DocumentKind, DocumentListRow, DocumentStatus, DocumentVersion,
    Recipient, Signature,
};
use crate::documents::render::{
    attachment_name, render_recipient_page, PageContext, PageRecipient, PageState,
};
use crate::documents::send::preview_recipients;
use crate::documents::store::{
    document_record, get_document, latest_version, list_documents, read_version_pdf, DocumentFilter,
};
use crate::invoicing::clients::get_client;

use super::super::error::{ApiError, ApiResult};
use super::super::extract::ApiPath;
use super::super::state::AppState;
use super::{not_found_because, with_conn, with_conn_api};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/documents", get(list))
        .route("/documents/{id}", get(detail))
        .route("/documents/{id}/preview", get(preview_html))
        .route("/documents/{id}/preview.pdf", get(preview_pdf))
        .route("/document-kinds", get(kinds))
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// One document with every version and what may be done to it next.
///
/// The document's own fields are flattened, so its `token` stays skipped.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentDetail {
    #[serde(flatten)]
    document: Document,
    client_name: String,
    versions: Vec<VersionDetail>,
    can_edit: bool,
    can_send: bool,
    can_revise: bool,
    can_accept: bool,
    can_request_changes: bool,
    can_decline: bool,
    can_countersign: bool,
    can_withdraw: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionDetail {
    #[serde(flatten)]
    version: DocumentVersion,
    recipients: Vec<RecipientDetail>,
    signatures: Vec<Signature>,
    change_requests: Vec<ChangeRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecipientDetail {
    #[serde(flatten)]
    recipient: Recipient,
    /// The recipient's published page, or `null` when the version was never
    /// sent or no documents base URL is configured.
    page_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// The documents base a page address is built on, when one is configured and
/// would produce a working link — the check `build_document_clients` makes, so
/// the read path and the send path agree about which addresses are real.
fn documents_base() -> Option<String> {
    let cfg = crate::settings::documents_config();
    let base = crate::settings::derive_documents_base(
        cfg.documents_base_url.as_deref(),
        cfg.invoicing.public_base_url.as_deref(),
    )?;
    crate::invoicing::r2::validate_public_base_url(&base).ok()?;
    Some(base)
}

fn detail_for(conn: &Connection, id: i64) -> ApiResult<DocumentDetail> {
    let record =
        document_record(conn, id).map_err(|e| not_found_because(e, "document_not_found"))?;
    let status = record.document.status;
    let client_active = ensure_client_active_for_documents(conn, record.document.client_id).is_ok();
    let base = documents_base();
    let token = record.document.token.clone();

    let versions = record
        .versions
        .into_iter()
        .map(|v| {
            let sent = v.version.sent_at.is_some();
            let recipients = v
                .recipients
                .into_iter()
                .map(|recipient| RecipientDetail {
                    page_url: base.as_deref().filter(|_| sent).map(|base| {
                        crate::invoicing::r2::document_page_url(base, &token, &recipient.token)
                    }),
                    recipient,
                })
                .collect();
            VersionDetail {
                version: v.version,
                recipients,
                signatures: v.signatures,
                change_requests: v.change_requests,
            }
        })
        .collect();

    Ok(DocumentDetail {
        document: record.document,
        client_name: record.client_name,
        versions,
        can_edit: can(status, Action::Edit),
        can_send: can(status, Action::Send) && client_active,
        can_revise: can(status, Action::Revise) && client_active,
        can_accept: can(status, Action::Accept),
        can_request_changes: can(status, Action::RequestChanges),
        can_decline: can(status, Action::Decline),
        can_countersign: can(status, Action::Countersign),
        can_withdraw: can(status, Action::Withdraw),
    })
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// The list filters, taken as strings so a malformed one lands in the error
/// envelope instead of axum's plain-text `Query` rejection.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    client_id: Option<String>,
    status: Option<String>,
    kind: Option<String>,
}

fn parse_status(value: &str) -> ApiResult<DocumentStatus> {
    DocumentStatus::parse(value).ok_or_else(|| {
        let valid: Vec<&str> = DocumentStatus::ALL.iter().map(|s| s.as_str()).collect();
        ApiError::bad_request(format!(
            "Unknown status: {value}. Use one of: {}.",
            valid.join(", ")
        ))
    })
}

async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> ApiResult<Json<Vec<DocumentListRow>>> {
    let client_id = query
        .client_id
        .as_deref()
        .map(|value| {
            value.parse::<i64>().map_err(|_| {
                ApiError::bad_request(format!(
                    "Invalid `clientId`: expected a client id, got \"{value}\"."
                ))
            })
        })
        .transpose()?;
    let filter = DocumentFilter {
        client_id,
        status: query.status.as_deref().map(parse_status).transpose()?,
        kind: query.kind,
    };

    let rows = with_conn_api(&state, move |conn| {
        if let Some(id) = client_id {
            crate::invoicing::clients::ensure_client_exists(conn, id)
                .map_err(|e| not_found_because(e, "client_not_found"))?;
        }
        Ok(list_documents(conn, &filter)?)
    })
    .await?;
    Ok(Json(rows))
}

async fn detail(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Json<DocumentDetail>> {
    let detail = with_conn_api(&state, move |conn| detail_for(conn, id)).await?;
    Ok(Json(detail))
}

async fn kinds(State(state): State<AppState>) -> ApiResult<Json<Vec<DocumentKind>>> {
    let kinds = with_conn(&state, |conn| list_kinds(conn, false)).await?;
    Ok(Json(kinds))
}

// ---------------------------------------------------------------------------
// Preview
// ---------------------------------------------------------------------------

/// The signer's page for the latest version, rendered through the functions a
/// send publishes through, with the PDF link pointed at the preview route.
fn render_preview(conn: &Connection, id: i64) -> ApiResult<String> {
    let document =
        get_document(conn, id).map_err(|e| not_found_because(e, "document_not_found"))?;
    let version = latest_version(conn, id)?;
    let client = get_client(conn, document.client_id)
        .map_err(|e| not_found_because(e, "client_not_found"))?;
    let company = crate::invoicing::wiring::company_name(conn);
    let response_url = crate::settings::documents_config().document_response_url;
    let pdf_href = format!("/api/documents/{id}/preview.pdf");
    let ctx = PageContext {
        company: &company,
        client_name: &client.name,
        kind: &document.kind,
        title: &document.title,
        token: &document.token,
        version: version.number,
        checksum: &version.checksum,
        pdf_href: &pdf_href,
    };
    let stand_ins = preview_recipients(conn, document.client_id);
    let (label, role, name) = &stand_ins[0];
    let signer = PageRecipient {
        token: label,
        role: *role,
        name,
    };
    Ok(render_recipient_page(
        &ctx,
        &signer,
        &PageState::Open {
            response_url: response_url.as_deref(),
        },
    ))
}

async fn preview_html(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Response> {
    let html = with_conn_api(&state, move |conn| render_preview(conn, id)).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CONTENT_SECURITY_POLICY, "sandbox"),
            (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
        ],
        html,
    )
        .into_response())
}

async fn preview_pdf(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
) -> ApiResult<Response> {
    let (bytes, filename) = with_conn_api(&state, {
        let state = state.clone();
        move |conn| {
            let document =
                get_document(conn, id).map_err(|e| not_found_because(e, "document_not_found"))?;
            let version = latest_version(conn, id)?;
            let bytes = read_version_pdf(&state.data_dir(), &version)?;
            Ok((bytes, attachment_name(&document.title, version.number)))
        }
    })
    .await?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{filename}\""),
            ),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use crate::documents::testing::{fixture_pdf, sent_document_with_fakes};
    use crate::server::testutil::*;
    use axum::http::{header, StatusCode};

    fn ids(body: &serde_json::Value) -> Vec<i64> {
        body.as_array()
            .expect("a bare array")
            .iter()
            .map(|row| row["id"].as_i64().unwrap())
            .collect()
    }

    #[tokio::test]
    async fn list_filters_and_404s_an_unknown_client() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let all = ok_json(&app, "/api/documents", &token).await;
        assert_eq!(ids(&all), vec![1], "{all}");
        assert_eq!(all[0]["title"], "Website rebuild");
        assert_eq!(all[0]["kind"], "Proposal");
        assert_eq!(all[0]["clientName"], "Acme Co");
        assert_eq!(all[0]["status"], "draft");
        assert_eq!(all[0]["latestVersion"], 1);

        for (query, expected) in [
            ("clientId=1", vec![1]),
            ("clientId=2", vec![]),
            ("status=draft", vec![1]),
            ("status=sent", vec![]),
            ("kind=Proposal", vec![1]),
            ("kind=Agreement", vec![]),
            ("clientId=1&status=draft&kind=Proposal", vec![1]),
        ] {
            let body = ok_json(&app, &format!("/api/documents?{query}"), &token).await;
            assert_eq!(ids(&body), expected, "{query}: {body}");
        }

        let (status, body) = get_json(&app, "/api/documents?clientId=9999", &token).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_not_found");

        let (status, body) = get_json(&app, "/api/documents?clientId=acme", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("clientId"),
            "{body}"
        );

        let (status, body) = get_json(&app, "/api/documents?status=signed", &token).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("changes_requested"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn an_unknown_document_is_a_404_naming_the_document() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for uri in [
            "/api/documents/9999",
            "/api/documents/9999/preview",
            "/api/documents/9999/preview.pdf",
        ] {
            let (status, body) = get_json(&app, uri, &token).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
            assert_eq!(
                body["error"]["details"]["reason"], "document_not_found",
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn detail_never_carries_a_token() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let body = ok_json(&app, "/api/documents/1", &token).await;
        let text = body.to_string();
        assert!(body.get("token").is_none());
        assert!(body["versions"][0].get("filePath").is_none());
        let doc_token: String = crate::db::open_connection(&db_path, None)
            .unwrap()
            .query_row("SELECT token FROM documents WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert!(!text.contains(&doc_token));
    }

    #[tokio::test]
    async fn detail_can_flags_are_the_guards_called() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let body = ok_json(&app, "/api/documents/1", &token).await;
        assert_eq!(body["status"], "draft", "{body}");
        assert_eq!(body["clientName"], "Acme Co", "{body}");
        assert_eq!(body["versions"][0]["number"], 1, "{body}");
        for (flag, expected) in [
            ("canEdit", true),
            ("canSend", true),
            ("canWithdraw", true),
            ("canRevise", false),
            ("canAccept", false),
            ("canRequestChanges", false),
            ("canDecline", false),
            ("canCountersign", false),
        ] {
            assert_eq!(body[flag], expected, "{flag}: {body}");
        }

        let conn = crate::db::open_connection(&db_path, None).unwrap();
        crate::invoicing::clients::archive_client(&conn, 1, "2026-10-05").unwrap();
        drop(conn);
        let archived = ok_json(&app, "/api/documents/1", &token).await;
        assert_eq!(archived["canSend"], false, "{archived}");
        assert_eq!(archived["canEdit"], true, "{archived}");
    }

    #[tokio::test]
    async fn kinds_lists_only_active_kinds() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let names = |body: &serde_json::Value| -> Vec<String> {
            body.as_array()
                .unwrap()
                .iter()
                .map(|k| k["name"].as_str().unwrap().to_string())
                .collect()
        };
        let body = ok_json(&app, "/api/document-kinds", &token).await;
        assert_eq!(
            names(&body),
            ["Proposal", "Estimate", "Agreement"],
            "{body}"
        );

        let conn = crate::db::open_connection(&db_path, None).unwrap();
        let estimate = body[1]["id"].as_i64().unwrap();
        crate::documents::kinds::deactivate_kind(&conn, estimate).unwrap();
        drop(conn);

        let body = ok_json(&app, "/api/document-kinds", &token).await;
        assert_eq!(names(&body), ["Proposal", "Agreement"], "{body}");
        assert_eq!(body[0]["active"], true, "{body}");
    }

    #[tokio::test]
    async fn preview_is_sandboxed_and_names_the_pdf_route() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let response = get_response(&app, "/api/documents/1/preview", &token).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(content_type(&response), "text/html; charset=utf-8");
        assert_eq!(
            header_str(&response, header::CONTENT_SECURITY_POLICY),
            "sandbox"
        );
        assert_eq!(header_str(&response, header::X_FRAME_OPTIONS), "SAMEORIGIN");
        let html = body_string(response).await;
        assert!(
            html.contains("href=\"/api/documents/1/preview.pdf\""),
            "{html}"
        );
        assert!(html.contains("Website rebuild"), "{html}");
        assert!(html.contains("Acme Co"), "{html}");
    }

    #[tokio::test]
    async fn preview_pdf_answers_the_filed_bytes() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let response = get_response(&app, "/api/documents/1/preview.pdf", &token).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(content_type(&response), "application/pdf");
        assert_eq!(
            header_str(&response, header::CONTENT_DISPOSITION),
            "inline; filename=\"Website-rebuild-v1.pdf\""
        );
        assert_eq!(body_bytes(response).await, fixture_pdf("seed"));
    }

    #[tokio::test]
    async fn a_sent_document_carries_page_urls_but_no_token_field() {
        let _config = TempConfig::new();
        let mut settings = crate::settings::load_settings();
        settings.public_base_url = Some("https://billing.example.test/i".to_string());
        crate::settings::save_settings(&settings).expect("settings");
        let (_dir, db_path) = seeded_db();
        let conn = crate::db::open_connection(&db_path, None).unwrap();
        let (id, _, _) = sent_document_with_fakes(&conn, db_path.parent().unwrap());
        drop(conn);
        let (app, token) = app_for(&db_path);
        let body = ok_json(&app, &format!("/api/documents/{id}"), &token).await;
        let recipient = &body["versions"][0]["recipients"][0];
        assert!(body.get("token").is_none() && recipient.get("token").is_none());
        assert!(recipient["pageUrl"]
            .as_str()
            .unwrap()
            .ends_with("/index.html"));
        assert!(recipient["pageUrl"]
            .as_str()
            .unwrap()
            .starts_with("https://billing.example.test/d/"));
        assert_eq!(body["status"], "sent", "{body}");
        assert_eq!(body["canSend"], false, "{body}");
        assert_eq!(body["canRevise"], true, "{body}");
    }
}
