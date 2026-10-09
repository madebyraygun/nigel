//! Preview and send share one render path: what `nigel document preview` writes
//! is built by the same functions that build what a client receives.
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Serialize, Serializer};

use super::guards::{ensure_allowed, ensure_client_active_for_documents, Action};
use super::model::{gen_document_token, DocumentStatus, NewRecipient, RecipientRole};
use super::record::{freeze_recipients, mark_sent, unfreeze};
use super::render::{
    attachment_name, email_subject, relative_pdf_href, render_document_email_text,
    render_document_pages, PageContext, PageRecipient, PageState,
};
use super::store::{get_document, latest_version, read_version_pdf};
use super::wire::{manifest_for, Manifest, ManifestState};
use crate::db::get_metadata;
use crate::error::{NigelError, Result};
use crate::invoicing::clients::{get_client, list_contacts};
use crate::invoicing::gateway::{
    Attachment, DocumentPublisher, Mailer, OutgoingMail, ResponseSource,
};
use crate::invoicing::logo::{fingerprint, COMPANY_LOGO_KEY, PUBLISHED_LOGO_KEY};
use crate::invoicing::mailgun::format_address;
use crate::invoicing::render::usable_logo;
use crate::invoicing::send::StepOutcome;

pub struct PreviewFiles {
    pub dir: PathBuf,
    pub pages: Vec<PathBuf>,
    pub pdf: PathBuf,
}

/// The pages a preview stands in for: the client's billing contact as the
/// signer, and a placeholder collaborator so both page shapes are visible.
pub fn preview_recipients(
    conn: &Connection,
    client_id: i64,
) -> Vec<(String, RecipientRole, String)> {
    let signer = list_contacts(conn, client_id)
        .ok()
        .and_then(|contacts| contacts.into_iter().find(|c| c.is_billing))
        .and_then(|c| c.name)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Signer".to_string());
    vec![
        ("signer".into(), RecipientRole::Signer, signer),
        (
            "collaborator".into(),
            RecipientRole::Collaborator,
            "Collaborator".into(),
        ),
    ]
}

pub fn write_preview(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    response_url: Option<&str>,
    out_dir: &Path,
) -> Result<PreviewFiles> {
    let document = get_document(conn, id)?;
    let version = latest_version(conn, id)?;
    let pdf_bytes = read_version_pdf(data_dir, &version)?;
    let client = get_client(conn, document.client_id)?;
    let href = relative_pdf_href(version.number);
    let logo = page_logo_src(conn, None);
    let ctx = PageContext {
        company,
        client_name: &client.name,
        kind: &document.kind,
        title: &document.title,
        token: &document.token,
        version: version.number,
        checksum: &version.checksum,
        pdf_href: &href,
        logo_src: logo.as_deref(),
    };
    let stand_ins = preview_recipients(conn, document.client_id);
    let recipients: Vec<PageRecipient<'_>> = stand_ins
        .iter()
        .map(|(label, role, name)| PageRecipient {
            token: label,
            role: *role,
            name,
        })
        .collect();
    let pages = render_document_pages(&ctx, &recipients, &PageState::Open { response_url });

    let dir = out_dir.join(format!("document-{id}"));
    let mut written = Vec::new();
    for (label, html) in pages {
        let path = dir.join(&label).join("index.html");
        write_file(&path, html.as_bytes())?;
        written.push(path);
    }
    let pdf = dir
        .join(format!("v{}", version.number))
        .join("document.pdf");
    write_file(&pdf, &pdf_bytes)?;
    Ok(PreviewFiles {
        dir,
        pages: written,
        pdf,
    })
}

/// The stages of a document send, in execution order. `Config` belongs to the
/// caller, which builds the publisher, mailer and response source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSendStep {
    Config,
    Load,
    Render,
    Freeze,
    Publish,
    Manifest,
    Email,
    Record,
}

impl DocumentSendStep {
    pub const ALL: [Self; 8] = [
        Self::Config,
        Self::Load,
        Self::Render,
        Self::Freeze,
        Self::Publish,
        Self::Manifest,
        Self::Email,
        Self::Record,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Load => "load",
            Self::Render => "render",
            Self::Freeze => "freeze",
            Self::Publish => "publish",
            Self::Manifest => "manifest",
            Self::Email => "email",
            Self::Record => "record",
        }
    }
}

impl Serialize for DocumentSendStep {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// One recipient's personal page, as it was emailed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipientLink {
    pub role: RecipientRole,
    pub name: String,
    pub email: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct DocumentSendOutcome {
    pub version: i64,
    pub links: Vec<RecipientLink>,
    pub steps: Vec<(DocumentSendStep, StepOutcome)>,
    pub warnings: Vec<String>,
}

/// A send that stopped, and where. The version is left unsent: its recipients
/// are removed and an opened manifest closed, unless `cleanup_warnings` says
/// otherwise. The PDF and pages already published stay in R2. `emailed` lists
/// the addresses that already received their link.
#[derive(Debug)]
pub struct DocumentSendFailure {
    pub step: DocumentSendStep,
    pub completed: Vec<DocumentSendStep>,
    pub emailed: Vec<String>,
    pub document_status: Option<DocumentStatus>,
    pub cleanup_warnings: Vec<String>,
    pub source: NigelError,
}

/// The letterhead logo's `<img src>` for a recipient page.
///
/// The address invoices already published the current logo at, when the
/// recorded object is this exact image and lives on the same origin the
/// document pages are served from; otherwise the image inline as a `data:` URI.
/// A recipient page is opened in a browser rather than shown as an email body,
/// so the inline image renders there and nothing is uploaded on a document's
/// account. `documents_base` is `None` for a preview, which always inlines.
pub(crate) fn page_logo_src(conn: &Connection, documents_base: Option<&str>) -> Option<String> {
    let stored = get_metadata(conn, COMPANY_LOGO_KEY).unwrap_or_default();
    let logo = usable_logo(&stored)?;
    let published = documents_base.and_then(|base| {
        let recorded = get_metadata(conn, PUBLISHED_LOGO_KEY)?;
        let (print, url) = recorded.split_once(' ')?;
        (print == fingerprint(&logo.bytes) && origin(url).is_some() && origin(url) == origin(base))
            .then(|| url.to_string())
    });
    Some(published.unwrap_or_else(|| format!("data:{};base64,{}", logo.mime, logo.base64)))
}

/// `scheme://host[:port]` of an absolute URL.
fn origin(url: &str) -> Option<&str> {
    let after = url.find("://")? + 3;
    let end = url[after..].find('/').map_or(url.len(), |i| after + i);
    (end > after).then(|| &url[..end])
}

pub struct SendContext<'a> {
    pub data_dir: &'a Path,
    pub company: &'a str,
    pub response_url: Option<&'a str>,
    pub today: &'a str,
}

/// The client's billing contact as the signer, or the refusal that asks for
/// one to be named.
pub fn default_signer(conn: &Connection, client_id: i64) -> Result<NewRecipient> {
    let billing = list_contacts(conn, client_id)?
        .into_iter()
        .find(|c| c.is_billing);
    let Some(contact) = billing else {
        let client = get_client(conn, client_id)?;
        return Err(NigelError::Conflict {
            code: "client_missing_email",
            message: format!("client '{}' has no email", client.name),
        });
    };
    match contact.name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => Ok(NewRecipient {
            role: RecipientRole::Signer,
            name: name.to_string(),
            email: contact.email,
        }),
        _ => Err(NigelError::Conflict {
            code: "signer_name_required",
            message: format!(
                "The billing contact {} has no name to type when accepting; name the signer as \"Name <{}>\".",
                contact.email, contact.email
            ),
        }),
    }
}

#[derive(Default)]
struct Trace {
    steps: Vec<(DocumentSendStep, StepOutcome)>,
    emailed: Vec<String>,
    frozen_version: Option<i64>,
    open_manifest: Option<(String, Manifest)>,
}

impl Trace {
    fn done(&mut self, step: DocumentSendStep) {
        self.steps.push((step, StepOutcome::Ok));
    }

    fn roll_back<R: ResponseSource>(&mut self, conn: &Connection, source: &R) -> Vec<String> {
        let mut warnings = Vec::new();
        if let Some(version_id) = self.frozen_version {
            if let Err(e) = unfreeze(conn, version_id) {
                warnings.push(format!(
                    "Warning: the recipients recorded for this send could not be removed ({e})."
                ));
            }
        }
        if let Some((token, mut manifest)) = self.open_manifest.take() {
            manifest.state = ManifestState::Closed;
            if let Err(e) = source.put_manifest(&token, &manifest) {
                warnings.push(format!(
                    "Warning: the response manifest for this send could not be closed ({e}); responses to the links already emailed will be refused by Nigel but may reach the Worker."
                ));
            }
        }
        warnings
    }
}

/// Send a document's latest version to one signer and any collaborators,
/// reporting what each step did. Any failure before the send is recorded is
/// rolled back.
#[allow(clippy::result_large_err)]
pub fn send_document_traced<P: DocumentPublisher, M: Mailer, R: ResponseSource>(
    conn: &Connection,
    document_id: i64,
    recipients: &[NewRecipient],
    ctx: &SendContext<'_>,
    publisher: &P,
    mailer: &M,
    source: &R,
) -> std::result::Result<DocumentSendOutcome, DocumentSendFailure> {
    let mut trace = Trace::default();
    match run(
        conn,
        document_id,
        recipients,
        ctx,
        publisher,
        mailer,
        source,
        &mut trace,
    ) {
        Ok(outcome) => Ok(outcome),
        Err((step, error)) => {
            let cleanup_warnings = trace.roll_back(conn, source);
            Err(DocumentSendFailure {
                step,
                completed: trace.steps.iter().map(|(s, _)| *s).collect(),
                emailed: trace.emailed,
                document_status: get_document(conn, document_id).ok().map(|d| d.status),
                cleanup_warnings,
                source: error,
            })
        }
    }
}

pub fn send_document<P: DocumentPublisher, M: Mailer, R: ResponseSource>(
    conn: &Connection,
    document_id: i64,
    recipients: &[NewRecipient],
    ctx: &SendContext<'_>,
    publisher: &P,
    mailer: &M,
    source: &R,
) -> Result<DocumentSendOutcome> {
    send_document_traced(
        conn,
        document_id,
        recipients,
        ctx,
        publisher,
        mailer,
        source,
    )
    .map_err(|f| f.source)
}

#[allow(clippy::too_many_arguments)]
fn run<P: DocumentPublisher, M: Mailer, R: ResponseSource>(
    conn: &Connection,
    document_id: i64,
    recipients: &[NewRecipient],
    ctx: &SendContext<'_>,
    publisher: &P,
    mailer: &M,
    source: &R,
    trace: &mut Trace,
) -> std::result::Result<DocumentSendOutcome, (DocumentSendStep, NigelError)> {
    use DocumentSendStep as Step;

    let load = |e| (Step::Load, e);
    let document = get_document(conn, document_id).map_err(load)?;
    ensure_allowed(document_id, document.status, Action::Send).map_err(load)?;
    ensure_client_active_for_documents(conn, document.client_id).map_err(load)?;
    let version = latest_version(conn, document_id).map_err(load)?;
    let pdf = read_version_pdf(ctx.data_dir, &version).map_err(load)?;
    let client = get_client(conn, document.client_id).map_err(load)?;
    trace.done(Step::Load);

    let set: Vec<(NewRecipient, String)> = recipients
        .iter()
        .map(|r| {
            let trimmed = NewRecipient {
                role: r.role,
                name: r.name.trim().to_string(),
                email: r.email.trim().to_string(),
            };
            (trimmed, gen_document_token())
        })
        .collect();
    let href = relative_pdf_href(version.number);
    let logo = page_logo_src(conn, Some(publisher.public_base()));
    let page_ctx = PageContext {
        company: ctx.company,
        client_name: &client.name,
        kind: &document.kind,
        title: &document.title,
        token: &document.token,
        version: version.number,
        checksum: &version.checksum,
        pdf_href: &href,
        logo_src: logo.as_deref(),
    };
    let page_recipients: Vec<PageRecipient<'_>> = set
        .iter()
        .map(|(r, token)| PageRecipient {
            token,
            role: r.role,
            name: &r.name,
        })
        .collect();
    let pages = render_document_pages(
        &page_ctx,
        &page_recipients,
        &PageState::Open {
            response_url: ctx.response_url,
        },
    );
    trace.done(Step::Render);

    trace.frozen_version = Some(version.id);
    let frozen = freeze_recipients(conn, version.id, &set).map_err(|e| (Step::Freeze, e))?;
    trace.done(Step::Freeze);

    let publish = |e| (Step::Publish, e);
    publisher
        .publish_pdf(&document.token, version.number, &pdf)
        .map_err(publish)?;
    let mut links = Vec::with_capacity(frozen.len());
    for r in &frozen {
        let html = pages
            .iter()
            .find(|(token, _)| *token == r.token)
            .map(|(_, html)| html)
            .ok_or_else(|| {
                publish(NigelError::Other(format!(
                    "No page was rendered for recipient {}",
                    r.email
                )))
            })?;
        let url = publisher
            .publish_page(&document.token, &r.token, html.as_bytes())
            .map_err(publish)?;
        links.push(RecipientLink {
            role: r.role,
            name: r.name.clone(),
            email: r.email.clone(),
            url,
        });
    }
    trace.done(Step::Publish);

    let manifest = manifest_for(&version, &frozen, ManifestState::Open);
    source
        .put_manifest(&document.token, &manifest)
        .map_err(|e| (Step::Manifest, e))?;
    trace.open_manifest = Some((document.token.clone(), manifest));
    trace.done(Step::Manifest);

    let filename = attachment_name(&document.title, version.number);
    for (r, link) in frozen.iter().zip(&links) {
        let recipient = PageRecipient {
            token: &r.token,
            role: r.role,
            name: &r.name,
        };
        let to = format_address(Some(&r.name), &r.email);
        let subject = email_subject(ctx.company, &document.title, r.role);
        let text = render_document_email_text(&page_ctx, &recipient, &link.url);
        mailer
            .send(&OutgoingMail {
                to: &to,
                cc: &[],
                subject: &subject,
                text: &text,
                attachment: Some(Attachment {
                    filename: &filename,
                    content_type: "application/pdf",
                    bytes: &pdf,
                }),
            })
            .map_err(|e| (Step::Email, e))?;
        trace.emailed.push(r.email.clone());
    }
    trace.done(Step::Email);

    mark_sent(conn, version.id, ctx.today).map_err(|e| (Step::Record, e))?;
    trace.done(Step::Record);

    Ok(DocumentSendOutcome {
        version: version.number,
        links,
        steps: std::mem::take(&mut trace.steps),
        warnings: Vec::new(),
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::{DocumentStatus, NewRecipient, RecipientRole};
    use crate::documents::store::recipients;
    use crate::documents::testing::{
        pat, sam, seed_client, seed_document, test_conn, FakeDocumentPublisher, FakeMailer,
        FakeResponseSource,
    };
    use crate::documents::wire::ManifestState;
    use crate::error::NigelError;

    #[test]
    fn preview_writes_pages_and_the_pdf_with_no_network_and_no_configuration() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let out = tempfile::tempdir().unwrap();
        let files = write_preview(&conn, dir.path(), id, "", None, out.path()).unwrap();
        assert_eq!(files.pages.len(), 2);
        let signer = std::fs::read_to_string(&files.pages[0]).unwrap();
        assert!(signer.contains("../v1/document.pdf") && !signer.contains("<form"));
        assert!(files.pdf.ends_with("document-1/v1/document.pdf"));
        assert!(files.pdf.exists());
    }

    #[cfg(feature = "pdf")]
    mod logo {
        use super::*;
        use crate::db::set_metadata;
        use crate::invoicing::document::parse_logo;

        fn store_logo(conn: &Connection) -> String {
            let uri = crate::pdf::logo_uri(400, 60);
            set_metadata(conn, COMPANY_LOGO_KEY, &uri).unwrap();
            uri
        }

        fn record_published(conn: &Connection, uri: &str, url: &str) {
            let bytes = parse_logo(uri).unwrap().expect("a logo").bytes;
            set_metadata(
                conn,
                PUBLISHED_LOGO_KEY,
                &format!("{} {url}", fingerprint(&bytes)),
            )
            .unwrap();
        }

        #[test]
        fn no_stored_logo_draws_no_image() {
            let (_dir, conn) = test_conn();
            assert_eq!(
                page_logo_src(&conn, Some(FakeDocumentPublisher::BASE)),
                None
            );
        }

        #[test]
        fn an_unpublished_logo_is_inlined() {
            let (_dir, conn) = test_conn();
            let uri = store_logo(&conn);
            assert_eq!(
                page_logo_src(&conn, Some(FakeDocumentPublisher::BASE)),
                Some(uri)
            );
        }

        #[test]
        fn the_logo_invoices_published_is_reused_on_the_same_origin() {
            let (_dir, conn) = test_conn();
            let uri = store_logo(&conn);
            let origin = origin(FakeDocumentPublisher::BASE).unwrap();
            let url = format!("{origin}/i/logo-abc.png");
            record_published(&conn, &uri, &url);
            assert_eq!(
                page_logo_src(&conn, Some(FakeDocumentPublisher::BASE)),
                Some(url)
            );
            assert_eq!(
                page_logo_src(&conn, None),
                Some(uri),
                "a preview always inlines"
            );
        }

        #[test]
        fn a_published_logo_on_another_origin_or_of_another_image_is_not_reused() {
            let (_dir, conn) = test_conn();
            let uri = store_logo(&conn);
            record_published(&conn, &uri, "https://old-bucket.example.net/i/logo-abc.png");
            assert_eq!(
                page_logo_src(&conn, Some(FakeDocumentPublisher::BASE)),
                Some(uri.clone())
            );

            let origin = origin(FakeDocumentPublisher::BASE).unwrap();
            set_metadata(
                &conn,
                PUBLISHED_LOGO_KEY,
                &format!("deadbeef {origin}/i/logo-old.png"),
            )
            .unwrap();
            assert_eq!(
                page_logo_src(&conn, Some(FakeDocumentPublisher::BASE)),
                Some(uri)
            );
        }

        #[test]
        fn a_preview_page_carries_the_logo_inline() {
            let (dir, conn) = test_conn();
            let uri = store_logo(&conn);
            let id = seed_document(
                &conn,
                dir.path(),
                seed_client(&conn, "Cedar Systems"),
                "Website rebuild",
            );
            let out = tempfile::tempdir().unwrap();
            let files = write_preview(&conn, dir.path(), id, "", None, out.path()).unwrap();
            let page = std::fs::read_to_string(&files.pages[0]).unwrap();
            assert!(page.contains(&format!("<header><p><img src=\"{uri}\" alt=\"\"")));
        }

        #[test]
        fn origins() {
            assert_eq!(
                origin("https://billing.example.com/d"),
                Some("https://billing.example.com")
            );
            assert_eq!(
                origin("https://billing.example.com"),
                Some("https://billing.example.com")
            );
            assert_eq!(origin("billing.example.com/d"), None);
            assert_eq!(origin("https:///d"), None);
        }
    }

    #[test]
    fn the_signer_is_the_billing_contact() {
        let (_dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let recipients = preview_recipients(&conn, client);
        assert_eq!(recipients[0].2, "Pat Example");
        assert_eq!(recipients[1].0, "collaborator");
    }

    fn ctx(dir: &Path) -> SendContext<'_> {
        SendContext {
            data_dir: dir,
            company: "Initech",
            response_url: Some("https://docs.example.test/d/respond"),
            today: "2026-10-05",
        }
    }

    #[test]
    fn a_send_publishes_one_page_per_recipient_writes_the_manifest_and_mails_each_their_link() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let (p, m, s) = (
            FakeDocumentPublisher::default(),
            FakeMailer::default(),
            FakeResponseSource::default(),
        );
        let out =
            send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s).unwrap();
        assert_eq!(
            out.steps.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            &DocumentSendStep::ALL[1..]
        );
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Sent
        );
        let doc = get_document(&conn, id).unwrap();
        assert!(p
            .keys()
            .contains(&format!("d/{}/v1/document.pdf", doc.token)));
        assert_eq!(
            p.keys()
                .iter()
                .filter(|k| k.ends_with("/index.html"))
                .count(),
            2
        );
        let manifest = s.last_manifest(&doc.token).unwrap();
        assert_eq!(
            (manifest.state, manifest.recipients.len()),
            (ManifestState::Open, 2)
        );
        let mails = m.sent.borrow();
        assert_eq!(
            mails[0].subject,
            "Please review and sign: Website rebuild for Initech"
        );
        assert_eq!(
            mails[1].subject,
            "Please review: Website rebuild for Initech"
        );
        assert_eq!(
            mails[0].attachment.as_ref().unwrap().0,
            "Website-rebuild-v1.pdf"
        );
        for (mail, link) in mails.iter().zip(&out.links) {
            assert!(mail.text.contains(&link.url) && link.url.ends_with("/index.html"));
        }
    }

    #[test]
    fn a_mail_failure_after_the_first_recipient_rolls_back_and_closes_the_manifest() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let (p, s) = (
            FakeDocumentPublisher::default(),
            FakeResponseSource::default(),
        );
        let m = FakeMailer {
            fail_on_call: Some(1),
            ..Default::default()
        };
        let failure =
            send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s)
                .unwrap_err();
        assert_eq!(failure.step, DocumentSendStep::Email);
        assert_eq!(failure.emailed, vec!["pat@cedar.test".to_string()]);
        assert_eq!(failure.document_status, Some(DocumentStatus::Draft));
        let v = latest_version(&conn, id).unwrap();
        assert!(v.sent_at.is_none() && recipients(&conn, v.id).unwrap().is_empty());
        let token = get_document(&conn, id).unwrap().token;
        assert_eq!(
            s.last_manifest(&token).unwrap().state,
            ManifestState::Closed
        );
    }

    fn assert_rolled_back(conn: &Connection, id: i64) {
        let v = latest_version(conn, id).unwrap();
        assert!(v.sent_at.is_none());
        assert!(recipients(conn, v.id).unwrap().is_empty());
        assert_eq!(
            get_document(conn, id).unwrap().status,
            DocumentStatus::Draft
        );
    }

    #[test]
    fn every_step_before_record_rolls_back_to_a_draft_with_no_recipients() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let cases: [(FakeDocumentPublisher, FakeResponseSource, DocumentSendStep); 2] = [
            (
                FakeDocumentPublisher {
                    fail_when_key_contains: Some("document.pdf".into()),
                    ..Default::default()
                },
                FakeResponseSource::default(),
                DocumentSendStep::Publish,
            ),
            (
                FakeDocumentPublisher::default(),
                FakeResponseSource {
                    fail_put: true,
                    ..Default::default()
                },
                DocumentSendStep::Manifest,
            ),
        ];
        for (p, s, step) in cases {
            let m = FakeMailer::default();
            let failure =
                send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s)
                    .unwrap_err();
            assert_eq!(failure.step, step);
            assert!(failure.emailed.is_empty() && m.sent.borrow().is_empty());
            assert_rolled_back(&conn, id);
        }
    }

    #[test]
    fn a_retry_after_a_failure_reuses_the_document_token_with_new_recipient_tokens() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let token = get_document(&conn, id).unwrap().token;
        let failing = FakeDocumentPublisher {
            fail_when_key_contains: Some("index.html".into()),
            ..Default::default()
        };
        let (m, s) = (FakeMailer::default(), FakeResponseSource::default());
        send_document_traced(
            &conn,
            id,
            &[pat(), sam()],
            &ctx(dir.path()),
            &failing,
            &m,
            &s,
        )
        .unwrap_err();
        let first_keys = failing.keys();
        let p = FakeDocumentPublisher::default();
        send_document_traced(&conn, id, &[pat(), sam()], &ctx(dir.path()), &p, &m, &s).unwrap();
        assert_eq!(get_document(&conn, id).unwrap().token, token);
        let v = latest_version(&conn, id).unwrap();
        for r in recipients(&conn, v.id).unwrap() {
            assert!(
                !first_keys.iter().any(|k| k.contains(&r.token)),
                "a recipient token was reused"
            );
            assert!(p.page(&token, &r.token).is_some());
        }
    }

    #[test]
    fn a_send_with_two_signers_stops_at_freeze_and_mails_nobody() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "Website rebuild",
        );
        let (p, m, s) = (
            FakeDocumentPublisher::default(),
            FakeMailer::default(),
            FakeResponseSource::default(),
        );
        let two = [
            pat(),
            NewRecipient {
                role: RecipientRole::Signer,
                ..sam()
            },
        ];
        let failure =
            send_document_traced(&conn, id, &two, &ctx(dir.path()), &p, &m, &s).unwrap_err();
        assert_eq!(failure.step, DocumentSendStep::Freeze);
        assert!(matches!(
            failure.source,
            NigelError::Conflict {
                code: "signer_count",
                ..
            }
        ));
        assert!(p.keys().is_empty() && m.sent.borrow().is_empty());
    }

    #[test]
    fn the_default_signer_is_the_named_billing_contact() {
        let (_dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        assert_eq!(default_signer(&conn, client).unwrap(), pat());
        let unnamed = crate::invoicing::clients::add_client(
            &conn,
            "Juniper Labs",
            Some("ap@juniper.test"),
            None,
            None,
        )
        .unwrap();
        assert!(matches!(
            default_signer(&conn, unnamed),
            Err(NigelError::Conflict {
                code: "signer_name_required",
                ..
            })
        ));
        let nobody =
            crate::invoicing::clients::add_client(&conn, "Globex", None, None, None).unwrap();
        assert!(matches!(
            default_signer(&conn, nobody),
            Err(NigelError::Conflict {
                code: "client_missing_email",
                ..
            })
        ));
    }
}
