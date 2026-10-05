//! Documents: the list, one document in full, the active kinds, the sandboxed
//! preview of the page a signer would receive, filing, editing and revising,
//! and the send.
//!
//! A filed or revised PDF arrives as multipart and goes through the uploads
//! spool like a statement does: its name is sanitized, its bytes must open with
//! the PDF header, and it is parked owner-only on disk, read back, handed to the
//! data layer and removed whatever the outcome.
//!
//! Every `can*` flag the detail response reports is `guards::can` — the table
//! the data layer enforces — plus the archived-client check for the two actions
//! whose data-layer functions make it, so a flag never disagrees with the 409
//! it predicts.
//!
//! No token crosses the wire as a field. A sent version's recipients carry the
//! computed `pageUrl`, and a send answers each recipient's link as it was
//! emailed; those are the only places a recipient's address appears.

use std::path::Path;

use axum::body::{Body, Bytes};
use axum::extract::multipart::MultipartRejection;
use axum::extract::{DefaultBodyLimit, Multipart, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::documents::guards::{can, ensure_client_active_for_documents, Action};
use crate::documents::kinds::list_kinds;
use crate::documents::lifecycle::revise_with_republish;
use crate::documents::model::{
    ChangeRequest, Document, DocumentKind, DocumentListRow, DocumentStatus, DocumentVersion,
    NewRecipient, Recipient, RecipientRole, Signature,
};
use crate::documents::render::{
    attachment_name, render_recipient_page, PageContext, PageRecipient, PageState,
};
use crate::documents::send::{
    default_signer, preview_recipients, send_document_traced, DocumentSendStep, RecipientLink,
    SendContext,
};
use crate::documents::store::{
    document_record, file_document, get_document, latest_version, list_documents, read_version_pdf,
    update_document, DocumentFilter, DocumentUpdate, NewDocument,
};
use crate::error::NigelError;
use crate::invoicing::clients::get_client;
use crate::invoicing::gateway::{DocumentPublisher, Mailer, ResponseSource};
use crate::invoicing::send::StepOutcome;
use crate::invoicing::wiring::{
    build_document_clients, company_name, optional_document_publisher, optional_response_source,
};
use crate::settings::DocumentsConfig;

use super::super::error::{ApiError, ApiResult};
use super::super::extract::{ApiJson, ApiPath};
use super::super::state::AppState;
use super::super::uploads::{self, UploadArea};
use super::imports::multipart_error;
use super::{not_found_because, with_conn, with_conn_api};

pub fn routes() -> Router<AppState> {
    let upload_limit = || DefaultBodyLimit::max(uploads::MAX_UPLOAD_BYTES);
    Router::new()
        .route(
            "/documents",
            get(list).merge(post(file).layer(upload_limit())),
        )
        .route("/documents/{id}", get(detail).patch(edit))
        .route("/documents/{id}/send", post(send))
        .route("/documents/{id}/revise", post(revise).layer(upload_limit()))
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

/// A document after an action whose follow-up work is best-effort: the action
/// stood, and `warnings` says what could not be published.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionResult {
    #[serde(flatten)]
    document: DocumentDetail,
    warnings: Vec<String>,
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
// Filing, editing and revising
// ---------------------------------------------------------------------------

/// The text fields and the one file of a multipart form.
struct Form {
    fields: Vec<(String, String)>,
    file: Option<(String, Bytes)>,
}

impl Form {
    fn field(&self, name: &str) -> ApiResult<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, value)| value.as_str())
            .ok_or_else(|| {
                ApiError::bad_request(format!("Expected a multipart form field named `{name}`."))
            })
    }

    /// The file, with its name sanitized and its bytes checked as a PDF.
    fn pdf(self) -> ApiResult<(String, Bytes)> {
        let Some((raw_name, bytes)) = self.file else {
            return Err(ApiError::bad_request(
                "Expected a multipart form with a file field named `file`.",
            ));
        };
        let filename = uploads::sanitize_filename_for(&raw_name, UploadArea::Document)
            .map_err(ApiError::bad_request)?;
        uploads::check_content(UploadArea::Document, &bytes).map_err(ApiError::bad_request)?;
        Ok((filename, bytes))
    }
}

async fn read_form(multipart: Result<Multipart, MultipartRejection>) -> ApiResult<Form> {
    let mut multipart = multipart.map_err(|r| ApiError::bad_request(r.body_text()))?;
    let mut form = Form {
        fields: Vec::new(),
        file: None,
    };
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        let name = field.name().unwrap_or_default().to_owned();
        match field.file_name().map(str::to_owned) {
            Some(filename) if form.file.is_none() => {
                let bytes = field.bytes().await.map_err(multipart_error)?;
                form.file = Some((filename, bytes));
            }
            Some(_) => {}
            None => {
                let value = field.text().await.map_err(multipart_error)?;
                form.fields.push((name, value));
            }
        }
    }
    Ok(form)
}

/// Park the upload in the spool, hand what is read back from disk to `work`,
/// and remove it whether or not `work` succeeded.
fn through_spool<T>(
    db_path: &Path,
    filename: &str,
    bytes: &[u8],
    work: impl FnOnce(&[u8]) -> ApiResult<T>,
) -> ApiResult<T> {
    let dir = uploads::uploads_dir(db_path);
    uploads::purge_stale(&dir, uploads::MAX_AGE);
    let stored = uploads::store(&dir, filename, bytes)?;
    let result = std::fs::read(&stored.path)
        .map_err(|e| ApiError::from(NigelError::from(e)))
        .and_then(|pdf| work(&pdf));
    uploads::delete(&dir, &stored.id);
    result
}

/// `POST /api/documents` — file a PDF for a client as a new draft.
async fn file(
    State(state): State<AppState>,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult<(StatusCode, Json<DocumentDetail>)> {
    let form = read_form(multipart).await?;
    let raw_client = form.field("clientId")?;
    let client_id = raw_client.trim().parse::<i64>().map_err(|_| {
        ApiError::bad_request(format!(
            "Invalid `clientId`: expected a client id, got \"{raw_client}\"."
        ))
    })?;
    let kind = form.field("kind")?.to_owned();
    let title = form.field("title")?.to_owned();
    let (filename, bytes) = form.pdf()?;
    let today = crate::clock::today();

    let detail = with_conn_api(&state, {
        let state = state.clone();
        move |conn| {
            through_spool(&state.db_path(), &filename, &bytes, |pdf| {
                let new = NewDocument {
                    client_id,
                    kind: &kind,
                    title: &title,
                };
                let id = file_document(conn, &state.data_dir(), &new, pdf, &today)
                    .map_err(|e| not_found_because(e, "client_not_found"))?;
                detail_for(conn, id)
            })
        }
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EditRequest {
    title: Option<String>,
    kind: Option<String>,
}

/// `PATCH /api/documents/{id}` — retitle or rekind a draft.
async fn edit(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(request): ApiJson<EditRequest>,
) -> ApiResult<Json<DocumentDetail>> {
    let today = crate::clock::today();
    let update = DocumentUpdate {
        title: request.title,
        kind: request.kind,
    };
    let detail = with_conn_api(&state, move |conn| {
        update_document(conn, id, &update, &today)
            .map_err(|e| not_found_because(e, "document_not_found"))?;
        detail_for(conn, id)
    })
    .await?;
    Ok(Json(detail))
}

/// `POST /api/documents/{id}/revise` — add a new draft version.
///
/// When an earlier version was sent, its pages are closed and republished
/// through whatever this installation configured. That is best-effort: the new
/// version stands either way, and what could not be published comes back as
/// `warnings`.
async fn revise(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    multipart: Result<Multipart, MultipartRejection>,
) -> ApiResult<Json<ActionResult>> {
    let (filename, bytes) = read_form(multipart).await?.pdf()?;
    let today = crate::clock::today();
    let config = crate::settings::documents_config();
    let publisher = optional_document_publisher(&config);
    let source = optional_response_source(&config);

    let result = with_conn_api(&state, {
        let state = state.clone();
        move |conn| {
            through_spool(&state.db_path(), &filename, &bytes, |pdf| {
                let company = company_name(conn);
                let outcome = revise_with_republish(
                    conn,
                    &state.data_dir(),
                    id,
                    pdf,
                    &today,
                    &company,
                    publisher.as_ref(),
                    source.as_ref(),
                )
                .map_err(|e| not_found_because(e, "document_not_found"))?;
                Ok(ActionResult {
                    document: detail_for(conn, id)?,
                    warnings: outcome.warnings,
                })
            })
        }
    })
    .await?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// Send
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendRecipient {
    name: String,
    email: String,
}

impl SendRecipient {
    fn into_recipient(self, role: RecipientRole) -> NewRecipient {
        NewRecipient {
            role,
            name: self.name,
            email: self.email,
        }
    }
}

/// The whole body of a send request. With no `signer`, the client's billing
/// contact signs.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SendRequest {
    /// Absent or `false` refuses the request and sends nothing.
    #[serde(default)]
    confirm: bool,
    signer: Option<SendRecipient>,
    #[serde(default)]
    collaborators: Vec<SendRecipient>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendStepResult {
    step: DocumentSendStep,
    outcome: StepOutcome,
}

/// A completed send: the refreshed detail, the trace, and each recipient's
/// page as it was emailed.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendResult {
    document: DocumentDetail,
    steps: Vec<SendStepResult>,
    links: Vec<RecipientLink>,
    config_warnings: Vec<String>,
    warnings: Vec<String>,
}

fn not_configured(missing: &[&'static str]) -> ApiError {
    ApiError::conflict(
        format!(
            "Sending documents is not configured: missing {} (set each one in settings.json or the matching NIGEL_ env var)",
            missing.join(", ")
        ),
        serde_json::json!({
            "reason": "send_not_configured",
            "step": DocumentSendStep::Config.as_str(),
            "missing": missing,
        }),
    )
}

fn misconfigured(err: NigelError) -> ApiError {
    match err {
        NigelError::Invalid(message) => ApiError::conflict(
            message,
            serde_json::json!({
                "reason": "send_misconfigured",
                "step": DocumentSendStep::Config.as_str(),
            }),
        ),
        other => other.into(),
    }
}

/// The documents base refused in the key-and-defect wording, never quoting the
/// configured value.
fn invalid_documents_base(config: &DocumentsConfig) -> Option<ApiError> {
    let base = crate::settings::derive_documents_base(
        config.documents_base_url.as_deref(),
        config.invoicing.public_base_url.as_deref(),
    )?;
    crate::invoicing::r2::validate_public_base_url(&base).err()?;
    let message = match config.documents_base_url {
        Some(_) => "documents_base_url is not an absolute http(s) address. Set it to the address \
             your bucket serves documents at, including the scheme — for example \
             https://billing.example.com/d."
            .to_string(),
        None => crate::invoicing::r2::PUBLIC_BASE_URL_DEFECT.to_string(),
    };
    Some(ApiError::conflict(
        message,
        serde_json::json!({
            "reason": "invalid_public_base_url",
            "step": DocumentSendStep::Config.as_str(),
        }),
    ))
}

/// `POST /api/documents/{id}/send` — the whole publish, inside the request,
/// refused without `confirm: true` before any setting is read.
async fn send(
    State(state): State<AppState>,
    ApiPath(id): ApiPath<i64>,
    ApiJson(request): ApiJson<SendRequest>,
) -> ApiResult<Json<SendResult>> {
    if !request.confirm {
        return Err(ApiError::bad_request(
            "Sending a document requires an explicit confirmation: post {\"confirm\": true}.",
        )
        .with_details(serde_json::json!({ "reason": "confirmation_required" })));
    }

    let config = crate::settings::documents_config();
    let status = crate::settings::documents_status(&config);
    if !status.send_configured {
        return Err(not_configured(&status.missing));
    }
    if let Some(err) = invalid_documents_base(&config) {
        return Err(err);
    }
    let company = with_conn(&state, |conn| Ok(company_name(conn))).await?;
    let clients = build_document_clients(config, &company).map_err(misconfigured)?;
    let today = crate::clock::today();
    let warnings = clients.warnings().to_vec();

    let mut result = with_conn_api(&state, {
        let state = state.clone();
        move |conn| {
            send_with(
                conn,
                &state.data_dir(),
                id,
                request,
                &company,
                clients.response_url(),
                &today,
                clients.publisher(),
                clients.mail(),
                clients.source(),
            )
        }
    })
    .await?;
    result.config_warnings = warnings;
    Ok(Json(result))
}

/// The send with its three collaborators passed in, so the orchestration runs
/// against fakes in tests and against the configured clients in `send`.
#[allow(clippy::too_many_arguments)]
fn send_with<P: DocumentPublisher, M: Mailer, R: ResponseSource>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    request: SendRequest,
    ctx_company: &str,
    response_url: Option<&str>,
    today: &str,
    publisher: &P,
    mailer: &M,
    source: &R,
) -> ApiResult<SendResult> {
    let at_load =
        |e| not_found_because(e, "document_not_found").at_document_step(DocumentSendStep::Load);
    let document = get_document(conn, id).map_err(at_load)?;
    let signer = match request.signer {
        Some(signer) => signer.into_recipient(RecipientRole::Signer),
        None => default_signer(conn, document.client_id).map_err(at_load)?,
    };
    let recipients: Vec<NewRecipient> = std::iter::once(signer)
        .chain(
            request
                .collaborators
                .into_iter()
                .map(|c| c.into_recipient(RecipientRole::Collaborator)),
        )
        .collect();

    let ctx = SendContext {
        data_dir,
        company: ctx_company,
        response_url,
        today,
    };
    let outcome = send_document_traced(conn, id, &recipients, &ctx, publisher, mailer, source)?;

    Ok(SendResult {
        document: detail_for(conn, id)?,
        steps: outcome
            .steps
            .into_iter()
            .map(|(step, outcome)| SendStepResult { step, outcome })
            .collect(),
        links: outcome.links,
        config_warnings: Vec::new(),
        warnings: outcome.warnings,
    })
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
    use crate::documents::testing::{
        fixture_pdf, sent_document_with_fakes, FakeDocumentPublisher, FakeMailer,
        FakeResponseSource,
    };
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

    fn new_pdf() -> Vec<u8> {
        fixture_pdf("filed over http")
    }

    async fn file_over_http(
        app: &axum::Router,
        token: &str,
        filename: &str,
        bytes: &[u8],
    ) -> (StatusCode, serde_json::Value) {
        post_multipart(
            app,
            "/api/documents",
            token,
            &[
                ("clientId", "1"),
                ("kind", "Estimate"),
                ("title", "Phase two"),
            ],
            Some((filename, bytes)),
        )
        .await
    }

    fn sent_document(db_path: &std::path::Path) -> i64 {
        let conn = crate::db::open_connection(db_path, None).unwrap();
        let (id, _, _) = sent_document_with_fakes(&conn, db_path.parent().unwrap());
        id
    }

    #[tokio::test]
    async fn filing_a_pdf_answers_the_draft_detail() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = file_over_http(&app, &token, "phase two.pdf", &new_pdf()).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(body["id"], 2, "{body}");
        assert_eq!(body["title"], "Phase two", "{body}");
        assert_eq!(body["kind"], "Estimate", "{body}");
        assert_eq!(body["clientName"], "Acme Co", "{body}");
        assert_eq!(body["status"], "draft", "{body}");
        assert_eq!(body["canEdit"], true, "{body}");
        assert_eq!(body["versions"][0]["number"], 1, "{body}");

        let response = get_response(&app, "/api/documents/2/preview.pdf", &token).await;
        assert_eq!(body_bytes(response).await, new_pdf());
    }

    #[tokio::test]
    async fn a_document_upload_named_pdf_holding_html_is_a_400() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let (status, json) = post_multipart(
            &app,
            "/api/documents",
            &token,
            &[("clientId", "1"), ("kind", "Proposal"), ("title", "Fake")],
            Some(("fake.pdf", b"<html>%PDF-1.7</html>")),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert!(json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not a PDF"));
    }

    #[tokio::test]
    async fn a_document_upload_with_a_csv_name_is_a_400() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = file_over_http(&app, &token, "march.csv", &new_pdf()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            body["error"]["message"].as_str().unwrap().contains(".pdf"),
            "{body}"
        );
        let all = ok_json(&app, "/api/documents", &token).await;
        assert_eq!(ids(&all), vec![1], "{all}");
    }

    #[tokio::test]
    async fn filing_the_same_pdf_twice_is_a_409_naming_the_document() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = file_over_http(&app, &token, "again.pdf", &fixture_pdf("seed")).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "duplicate_document");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("document #1"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn filing_for_an_archived_client_is_a_409_client_archived() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = crate::db::open_connection(&db_path, None).unwrap();
        crate::invoicing::clients::archive_client(&conn, 1, "2026-10-05").unwrap();
        drop(conn);
        let (app, token) = app_for(&db_path);

        let (status, body) = file_over_http(&app, &token, "phase two.pdf", &new_pdf()).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "client_archived");
    }

    #[tokio::test]
    async fn the_spool_is_empty_after_filing() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);
        let spool = crate::server::uploads::uploads_dir(&db_path);

        let (status, body) = file_over_http(&app, &token, "phase two.pdf", &new_pdf()).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        assert_eq!(std::fs::read_dir(&spool).unwrap().count(), 0);

        let (status, body) = file_over_http(&app, &token, "again.pdf", &new_pdf()).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(std::fs::read_dir(&spool).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn patch_outside_draft_is_a_409_with_the_data_layer_sentence() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let sent = sent_document(&db_path);
        let (app, token) = app_for(&db_path);

        let (status, body) = patch_json(
            &app,
            "/api/documents/1",
            &token,
            &serde_json::json!({ "title": "Website rebuild, phase one", "kind": "Agreement" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["title"], "Website rebuild, phase one", "{body}");
        assert_eq!(body["kind"], "Agreement", "{body}");

        let (status, body) = patch_json(
            &app,
            "/api/documents/1",
            &token,
            &serde_json::json!({ "status": "sent" }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, body) = patch_json(
            &app,
            &format!("/api/documents/{sent}"),
            &token,
            &serde_json::json!({ "title": "X" }),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        let expected = crate::documents::guards::ensure_allowed(
            sent,
            crate::documents::model::DocumentStatus::Sent,
            crate::documents::guards::Action::Edit,
        )
        .unwrap_err()
        .to_string();
        assert_eq!(body["error"]["message"], expected, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "document_wrong_state");
    }

    #[tokio::test]
    async fn revise_from_draft_is_a_409_document_wrong_state() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, body) = post_multipart(
            &app,
            "/api/documents/1/revise",
            &token,
            &[],
            Some(("v2.pdf", &new_pdf())),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"]["details"]["reason"], "document_wrong_state");
    }

    #[tokio::test]
    async fn revising_a_sent_document_answers_the_new_version_with_warnings_as_data() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let sent = sent_document(&db_path);
        let (app, token) = app_for(&db_path);
        let uri = format!("/api/documents/{sent}/revise");

        let (status, body) =
            post_multipart(&app, &uri, &token, &[], Some(("v2.pdf", b"<html></html>"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

        let (status, body) =
            post_multipart(&app, &uri, &token, &[], Some(("v2.pdf", &new_pdf()))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["id"], sent, "{body}");
        assert_eq!(body["versions"].as_array().unwrap().len(), 2, "{body}");
        assert_eq!(body["versions"][1]["number"], 2, "{body}");
        assert!(
            !body["warnings"].as_array().expect("warnings").is_empty(),
            "no publisher is configured, so the republish warns: {body}"
        );
        assert_eq!(
            std::fs::read_dir(crate::server::uploads::uploads_dir(&db_path))
                .unwrap()
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn the_multipart_routes_are_locked_with_the_database() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        encrypt(&db_path);
        let (app, token) = app_for(&db_path);

        for uri in ["/api/documents", "/api/documents/1/revise"] {
            let (status, body) = post_multipart(
                &app,
                uri,
                &token,
                &[("clientId", "1"), ("kind", "Proposal"), ("title", "X")],
                Some(("x.pdf", &new_pdf())),
            )
            .await;
            assert_eq!(status, StatusCode::LOCKED, "{uri}: {body}");
            assert_eq!(body["error"]["code"], "locked", "{uri}");
        }
    }

    fn send_request(collaborators: &[(&str, &str)]) -> super::SendRequest {
        super::SendRequest {
            confirm: true,
            signer: Some(super::SendRecipient {
                name: "Pat Example".into(),
                email: "pat@acme.test".into(),
            }),
            collaborators: collaborators
                .iter()
                .map(|(name, email)| super::SendRecipient {
                    name: (*name).into(),
                    email: (*email).into(),
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn send_without_confirmation_is_a_400_and_sends_nothing() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        for body in [
            serde_json::json!({}),
            serde_json::json!({ "confirm": false }),
        ] {
            let (status, json) = post_json(&app, "/api/documents/1/send", &token, &body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {json}");
            assert_eq!(json["error"]["details"]["reason"], "confirmation_required");
            assert_eq!(
                json["error"]["message"],
                "Sending a document requires an explicit confirmation: post {\"confirm\": true}."
            );
        }

        let detail = ok_json(&app, "/api/documents/1", &token).await;
        assert_eq!(detail["status"], "draft", "{detail}");
    }

    #[tokio::test]
    async fn send_with_no_configuration_is_a_409_naming_the_missing_keys() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let (app, token) = app_for(&db_path);

        let (status, json) = post_json(
            &app,
            "/api/documents/1/send",
            &token,
            &serde_json::json!({ "confirm": true }),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT, "{json}");
        let details = &json["error"]["details"];
        assert_eq!(details["reason"], "send_not_configured", "{json}");
        assert_eq!(details["step"], "config", "{json}");
        let missing = details["missing"]
            .as_array()
            .expect("the missing key names");
        assert_eq!(missing.len(), 9, "{json}");
        for key in ["mailgun_api_key", "r2_private_bucket", "documents_base_url"] {
            assert!(missing.contains(&serde_json::json!(key)), "{key}: {json}");
        }
        let detail = ok_json(&app, "/api/documents/1", &token).await;
        assert_eq!(detail["status"], "draft", "{detail}");
    }

    #[test]
    fn send_with_answers_the_detail_steps_and_links() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = crate::db::open_connection(&db_path, None).unwrap();
        let (publisher, mailer, source) = (
            FakeDocumentPublisher::default(),
            FakeMailer::default(),
            FakeResponseSource::default(),
        );

        let result = super::send_with(
            &conn,
            db_path.parent().unwrap(),
            1,
            send_request(&[("Sam Example", "sam@acme.test")]),
            "Initech",
            Some("https://docs.example.test/d/respond"),
            "2026-10-05",
            &publisher,
            &mailer,
            &source,
        )
        .expect("sends");

        let json = serde_json::to_value(&result).expect("serializes");
        let steps: Vec<&str> = json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["step"].as_str().unwrap())
            .collect();
        assert_eq!(
            steps,
            ["load", "render", "freeze", "publish", "manifest", "email", "record"]
        );
        assert!(json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["outcome"] == "ok"));
        assert_eq!(json["document"]["status"], "sent", "{json}");
        assert_eq!(json["document"]["canSend"], false, "{json}");
        let links = json["links"].as_array().expect("links");
        assert_eq!(links.len(), 2, "{json}");
        assert_eq!(links[0]["role"], "signer");
        assert_eq!(links[0]["email"], "pat@acme.test");
        assert_eq!(links[1]["role"], "collaborator");
        for link in links {
            assert!(
                link["url"].as_str().unwrap().ends_with("/index.html"),
                "{link}"
            );
            assert!(link.get("token").is_none(), "{link}");
        }
        assert_eq!(json["configWarnings"], serde_json::json!([]));
        assert_eq!(json["warnings"], serde_json::json!([]));
        assert_eq!(mailer.sent.borrow().len(), 2);
    }

    #[tokio::test]
    async fn a_mail_failure_is_a_502_naming_mailgun_and_who_was_emailed() {
        let _config = TempConfig::new();
        let (_dir, db_path) = seeded_db();
        let conn = crate::db::open_connection(&db_path, None).unwrap();
        let mailer = FakeMailer {
            fail_on_call: Some(1),
            ..FakeMailer::default()
        };

        let err = super::send_with(
            &conn,
            db_path.parent().unwrap(),
            1,
            send_request(&[("Sam Example", "sam@acme.test")]),
            "Initech",
            None,
            "2026-10-05",
            &FakeDocumentPublisher::default(),
            &mailer,
            &FakeResponseSource::default(),
        )
        .expect_err("the second email fails");

        let (status, json) = {
            use axum::response::IntoResponse;
            let response = err.into_response();
            let status = response.status();
            (status, json_body(response).await)
        };
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{json}");
        assert_eq!(json["error"]["code"], "upstream_failed", "{json}");
        let details = &json["error"]["details"];
        assert_eq!(details["service"], "mailgun", "{json}");
        assert_eq!(details["step"], "email", "{json}");
        assert_eq!(details["reason"], "send_failed", "{json}");
        assert_eq!(details["emailed"], serde_json::json!(["pat@acme.test"]));
        assert_eq!(
            details["completed"],
            serde_json::json!(["load", "render", "freeze", "publish", "manifest"])
        );
        assert_eq!(details["documentStatus"], "draft", "{json}");
        assert_eq!(details["cleanupWarnings"], serde_json::json!([]));
    }
}
