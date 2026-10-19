//! What follows a countersignature. The countersignature is committed first;
//! the signing record, the republished pages and the confirmation email then
//! follow best-effort, in that order, and whatever could not be done comes back
//! as warnings. Nothing here undoes the countersignature.
//!
//! The publisher and the mailer arrive as `Result`s: `Err` is why that client
//! is not available, which the warnings put into words.
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::lifecycle::republish_linking_record;
use super::render::{
    attachment_name, relative_signing_record_href, render_signed_email_text, signed_subject,
};
use super::signing_record::{
    generate_signing_record, signing_record_rel, GeneratedRecord, PDF_UNAVAILABLE,
    RECORD_UNAVAILABLE,
};
use super::store::{get_document, latest_sent_version, read_version_pdf, recipients};
use crate::error::{NigelError, Result};
use crate::invoicing::gateway::{
    Attachment, DocumentPublisher, Mailer, OutgoingMail, ResponseSource, Unavailable,
};
use crate::invoicing::mailgun::{ensure_message_fits, format_address};
use crate::invoicing::r2::document_page_url;

/// How to regenerate and republish a signing record and its pages, from either
/// surface.
pub fn republish_hint(id: i64) -> String {
    format!(
        "To republish, run `nigel document signing-record {id}` or choose Republish signing record on the document in the web UI."
    )
}

/// What the operator does when a signed copy was not emailed.
const SEND_YOURSELF: &str = "send the signed PDF and the signing record to the recipients yourself";

/// Generate the signing record, publish it, then republish the pages linking
/// it and email every recipient of the executed version a signed copy.
pub fn complete_execution<P: DocumentPublisher, R: ResponseSource, M: Mailer>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    publisher: std::result::Result<&P, &Unavailable>,
    source: Option<&R>,
    mailer: std::result::Result<&M, &Unavailable>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let record = match generate_signing_record(conn, data_dir, id, company) {
        Ok(Some(record)) => Some(record),
        Ok(None) => {
            warnings.push(RECORD_UNAVAILABLE.to_string());
            None
        }
        Err(e) => {
            warnings.push(format!(
                "Warning: the signing record could not be generated ({e}). {}",
                republish_hint(id)
            ));
            None
        }
    };
    publish_record_and_pages(
        conn,
        id,
        company,
        record.as_ref(),
        publisher,
        source,
        &mut warnings,
    );
    email_signed_copies(
        conn,
        data_dir,
        id,
        company,
        record.as_ref(),
        publisher.ok(),
        mailer,
        &mut warnings,
    );
    warnings
}

/// A regenerated record: where its local copy is, whether it was published,
/// and what else could not be republished.
#[derive(Debug)]
pub struct RegeneratedRecord {
    pub path: PathBuf,
    pub filename: String,
    /// The record itself reached the public bucket.
    pub published: bool,
    /// Why there was no publisher to try, when there was none.
    pub unavailable: Option<Unavailable>,
    pub warnings: Vec<String>,
}

impl RegeneratedRecord {
    /// A publish was expected and did not happen: the operator configured R2,
    /// at least in part, and the record is not up.
    pub fn failed(&self) -> bool {
        !self.published && self.unavailable.as_ref().is_none_or(|u| u.is_configured())
    }
}

/// Regenerate an executed document's signing record, write it locally, and
/// republish it and the pages linking it when a publisher is available.
/// Refused for a document that is not executed or a build without PDF
/// support; nothing is emailed.
pub fn regenerate_signing_record<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    publisher: std::result::Result<&P, &Unavailable>,
    source: Option<&R>,
) -> Result<RegeneratedRecord> {
    let record = generate_signing_record(conn, data_dir, id, company)?
        .ok_or_else(|| NigelError::Invalid(PDF_UNAVAILABLE.to_string()))?;
    let mut warnings = Vec::new();
    let published = publish_record_and_pages(
        conn,
        id,
        company,
        Some(&record),
        publisher,
        source,
        &mut warnings,
    );
    Ok(RegeneratedRecord {
        path: data_dir.join(signing_record_rel(id, record.version)),
        filename: record.filename,
        published,
        unavailable: publisher.err().cloned(),
        warnings,
    })
}

/// Publish the record, then the pages: a page links the record only once the
/// record is up, so no page ever points at an object that is not there.
/// Answers whether the record was published.
fn publish_record_and_pages<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    id: i64,
    company: &str,
    record: Option<&GeneratedRecord>,
    publisher: std::result::Result<&P, &Unavailable>,
    source: Option<&R>,
    warnings: &mut Vec<String>,
) -> bool {
    let mut published = false;
    match (record, publisher) {
        (Some(record), Ok(publisher)) => {
            let uploaded = get_document(conn, id).and_then(|document| {
                publisher.publish_signing_record(
                    &document.token,
                    record.version,
                    &record.filename,
                    &record.bytes,
                )
            });
            match uploaded {
                Ok(_) => published = true,
                Err(e) => warnings.push(format!(
                    "Warning: the countersignature is recorded, but the signing record could not be published ({e}). {}",
                    republish_hint(id)
                )),
            }
        }
        (Some(_), Err(unavailable)) => warnings.push(format!(
            "Warning: {}, so the signing record was not published; it is kept in the data directory. To publish it, {}. {}",
            unavailable.why(),
            unavailable.fix(),
            republish_hint(id)
        )),
        (None, _) => {}
    }
    let href = record.map(|r| relative_signing_record_href(r.version));
    let page_warnings = republish_linking_record(
        conn,
        id,
        company,
        href.as_deref().filter(|_| published),
        publisher.ok(),
        source,
    );
    warnings.extend(
        page_warnings
            .into_iter()
            .map(|w| format!("{w} {}", republish_hint(id))),
    );
    published
}

/// Email each recipient of the executed version, one message apiece, with the
/// signed PDF and the signing record attached, then sum up who was missed.
#[allow(clippy::too_many_arguments)]
fn email_signed_copies<P: DocumentPublisher, M: Mailer>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    record: Option<&GeneratedRecord>,
    publisher: Option<&P>,
    mailer: std::result::Result<&M, &Unavailable>,
    warnings: &mut Vec<String>,
) {
    let mailer = match mailer {
        Ok(mailer) => mailer,
        Err(unavailable) => {
            warnings.push(format!(
                "Warning: {}, so no signed copy was emailed. {SEND_YOURSELF_CAP}, and {} to email them for future documents.",
                unavailable.why(),
                unavailable.fix()
            ));
            return;
        }
    };
    let load = || -> Result<_> {
        let document = get_document(conn, id)?;
        let version = latest_sent_version(conn, id)?
            .ok_or_else(|| NigelError::Other(format!("Document #{id} has no sent version.")))?;
        let pdf = read_version_pdf(data_dir, &version)?;
        let frozen = recipients(conn, version.id)?;
        Ok((document, version, pdf, frozen))
    };
    let (document, version, pdf, frozen) = match load() {
        Ok(loaded) => loaded,
        Err(e) => {
            warnings.push(format!(
                "Warning: no signed copy was emailed, because the document could not be loaded ({e}). Check that its PDF is still in the data directory, then {SEND_YOURSELF}."
            ));
            return;
        }
    };
    if frozen.is_empty() {
        warnings.push(format!(
            "Warning: version {} has no recipients on record, so no signed copy was emailed.",
            version.number
        ));
        return;
    }
    let pdf_name = attachment_name(&document.title, version.number);
    let mut attachments = vec![Attachment {
        filename: &pdf_name,
        content_type: "application/pdf",
        bytes: &pdf,
    }];
    if let Some(record) = record {
        attachments.push(Attachment {
            filename: &record.filename,
            content_type: "application/pdf",
            bytes: &record.bytes,
        });
    }
    if let Err(e) = ensure_message_fits(&attachments, "") {
        warnings.push(format!(
            "Warning: no signed copy was emailed: {e} {SEND_YOURSELF_CAP} another way."
        ));
        return;
    }
    let subject = signed_subject(company, &document.title);
    let mut missed = Vec::new();
    for r in &frozen {
        let url = publisher.map(|p| document_page_url(p.public_base(), &document.token, &r.token));
        let text = render_signed_email_text(
            &r.name,
            &document.title,
            version.number,
            url.as_deref(),
            record.is_some(),
        );
        let to = format_address(Some(&r.name), &r.email);
        let sent = mailer.send(&OutgoingMail {
            to: &to,
            cc: &[],
            subject: &subject,
            text: &text,
            attachments: &attachments,
        });
        if let Err(e) = sent {
            warnings.push(format!(
                "Warning: the signed copy could not be emailed to {to} ({e})."
            ));
            missed.push(to);
        }
    }
    if !missed.is_empty() {
        warnings.push(format!(
            "Warning: emailed the signed copy to {} of {} recipients; not sent to {}. Send them the signed PDF and the signing record yourself.",
            frozen.len() - missed.len(),
            frozen.len(),
            missed.join(", ")
        ));
    }
}

const SEND_YOURSELF_CAP: &str =
    "Send the signed PDF and the signing record to the recipients yourself";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::DocumentStatus;
    use crate::documents::record::{record_countersign, record_manual_accept};
    use crate::documents::store::{latest_version, signatures};
    use crate::documents::testing::{
        sent_document_with_fakes, test_conn, FakeDocumentPublisher, FakeMailer, FakeResponseSource,
    };
    use crate::invoicing::gateway::UnavailableState;
    #[cfg(feature = "pdf")]
    use crate::invoicing::r2::document_signing_record_key;

    fn executed(conn: &Connection, dir: &Path) -> (i64, FakeDocumentPublisher, FakeResponseSource) {
        let (id, p, s) = sent_document_with_fakes(conn, dir);
        record_manual_accept(conn, id, "Pat Example", "2026-10-06").unwrap();
        record_countersign(conn, id, "Sam Example", "2026-10-07").unwrap();
        (id, p, s)
    }

    fn unset(service: &'static str) -> Unavailable {
        Unavailable {
            service,
            keys: &["a_key", "b_key"],
            state: UnavailableState::NotConfigured,
        }
    }

    fn still_executed(conn: &Connection, id: i64) {
        assert_eq!(
            get_document(conn, id).unwrap().status,
            DocumentStatus::Executed
        );
        let v = latest_version(conn, id).unwrap();
        assert_eq!(signatures(conn, v.id).unwrap().len(), 2);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn countersigning_publishes_the_record_before_the_pages_that_link_it() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let document = get_document(&conn, id).unwrap();
        let version = latest_version(&conn, id).unwrap();
        let signed_before = std::fs::read(dir.path().join(&version.file_path)).unwrap();
        let before = p.keys().len();
        let m = FakeMailer::default();

        let warnings =
            complete_execution(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s), Ok(&m));
        assert!(warnings.is_empty(), "{warnings:?}");

        let keys = p.keys()[before..].to_vec();
        let record_key = document_signing_record_key(&document.token, 1);
        assert_eq!(keys[0], record_key, "{keys:?}");
        assert!(keys[1..].iter().all(|k| k.ends_with("/index.html")));
        assert_eq!(keys.len(), 3);
        for r in recipients(&conn, version.id).unwrap() {
            let page = p.page(&document.token, &r.token).unwrap();
            assert!(page.contains(
                "<a class=\"button secondary\" href=\"../v1/signing-record.pdf\">Download the signing record</a>"
            ));
        }
        let published = p.objects.borrow()[before].1.clone();
        assert_eq!(
            std::fs::read(dir.path().join(signing_record_rel(id, 1))).unwrap(),
            published,
            "the local copy is the published one"
        );
        assert_eq!(
            std::fs::read(dir.path().join(&version.file_path)).unwrap(),
            signed_before,
            "the signed PDF is untouched"
        );
        assert_eq!(
            crate::documents::model::checksum_of(&signed_before),
            version.checksum
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn every_recipient_gets_the_signed_copy_and_the_record() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let document = get_document(&conn, id).unwrap();
        let version = latest_version(&conn, id).unwrap();
        let signed = std::fs::read(dir.path().join(&version.file_path)).unwrap();
        let m = FakeMailer::default();
        complete_execution(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s), Ok(&m));
        let record = std::fs::read(dir.path().join(signing_record_rel(id, 1))).unwrap();
        let sent = m.sent.borrow();
        let frozen = recipients(&conn, version.id).unwrap();
        assert_eq!(sent.len(), frozen.len());
        for (mail, r) in sent.iter().zip(&frozen) {
            assert_eq!(mail.to, format_address(Some(&r.name), &r.email));
            assert_eq!(mail.subject, "Signed: Website rebuild for Initech");
            assert_eq!(
                mail.attachments,
                vec![
                    ("Website-rebuild-v1.pdf".to_string(), signed.clone()),
                    (
                        "Website-rebuild-v1-signing-record.pdf".to_string(),
                        record.clone()
                    ),
                ]
            );
            assert!(mail.text.starts_with(&format!("Hello {},", r.name)));
            assert!(mail.text.contains("This document is now fully signed"));
            assert!(mail.text.contains("Website rebuild (version 1)"));
            assert!(mail.text.ends_with(&format!(
                "{}\n",
                document_page_url(FakeDocumentPublisher::BASE, &document.token, &r.token)
            )));
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn a_failed_record_upload_is_a_warning_the_pages_omit_the_link_and_a_regenerate_republishes() {
        let (dir, conn) = test_conn();
        let (id, _, s) = executed(&conn, dir.path());
        let failing = FakeDocumentPublisher {
            fail_when_key_contains: Some("signing-record.pdf".into()),
            ..Default::default()
        };
        let m = FakeMailer::default();
        let warnings = complete_execution(
            &conn,
            dir.path(),
            id,
            "Initech",
            Ok(&failing),
            Some(&s),
            Ok(&m),
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("signing record could not be published")
                && warnings[0].ends_with(&republish_hint(id)),
            "{warnings:?}"
        );
        assert!(republish_hint(id).contains(&format!("nigel document signing-record {id}")));
        assert!(republish_hint(id).contains("Republish signing record"));
        still_executed(&conn, id);
        let document = get_document(&conn, id).unwrap();
        let version = latest_version(&conn, id).unwrap();
        for r in recipients(&conn, version.id).unwrap() {
            let page = failing.page(&document.token, &r.token).unwrap();
            assert!(
                page.contains("Countersigned by Sam Example") && !page.contains("signing record")
            );
        }
        assert_eq!(m.sent.borrow().len(), 2, "the email still goes");

        let again =
            regenerate_signing_record(&conn, dir.path(), id, "Initech", Ok(&failing), Some(&s))
                .unwrap();
        assert!(!again.published && again.failed());

        let p = FakeDocumentPublisher::default();
        let regenerated =
            regenerate_signing_record(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s)).unwrap();
        assert!(
            regenerated.warnings.is_empty(),
            "{:?}",
            regenerated.warnings
        );
        assert!(regenerated.published && !regenerated.failed());
        assert_eq!(
            regenerated.filename,
            "Website-rebuild-v1-signing-record.pdf"
        );
        assert!(regenerated.path.exists());
        assert_eq!(p.keys()[0], document_signing_record_key(&document.token, 1));
        for r in recipients(&conn, version.id).unwrap() {
            assert!(p
                .page(&document.token, &r.token)
                .unwrap()
                .contains("Download the signing record"));
        }
        assert_eq!(m.sent.borrow().len(), 2, "a regenerate emails nobody");
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn without_a_publisher_the_record_stays_local_and_a_regenerate_is_not_a_failure() {
        let (dir, conn) = test_conn();
        let (id, _, s) = executed(&conn, dir.path());
        let r2 = unset("R2");
        let m = FakeMailer::default();
        let warnings = complete_execution::<FakeDocumentPublisher, _, _>(
            &conn,
            dir.path(),
            id,
            "Initech",
            Err(&r2),
            Some(&s),
            Ok(&m),
        );
        assert!(
            warnings[0].starts_with(
                "Warning: R2 is not configured, so the signing record was not published; it is kept in the data directory. To publish it, set a_key, b_key."
            ),
            "{warnings:?}"
        );
        assert!(
            warnings[1..]
                .iter()
                .all(|w| w.ends_with(&republish_hint(id))),
            "page warnings carry the hint: {warnings:?}"
        );
        assert!(
            m.sent
                .borrow()
                .iter()
                .all(|mail| !mail.text.contains("http")),
            "no link without a publisher"
        );
        let regenerated = regenerate_signing_record::<FakeDocumentPublisher, _>(
            &conn,
            dir.path(),
            id,
            "Initech",
            Err(&r2),
            Some(&s),
        )
        .unwrap();
        assert!(!regenerated.published && !regenerated.failed());
    }

    #[test]
    fn a_failed_send_names_the_address_and_a_summary_says_who_was_missed() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let m = FakeMailer {
            fail_on_call: Some(1),
            ..Default::default()
        };
        let warnings =
            complete_execution(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s), Ok(&m));
        let mail_warnings: Vec<_> = warnings.iter().filter(|w| w.contains("emailed")).collect();
        assert_eq!(
            mail_warnings,
            [
                "Warning: the signed copy could not be emailed to Sam Example <sam@cedar.test> (mailgun 500: boom).",
                "Warning: emailed the signed copy to 1 of 2 recipients; not sent to Sam Example <sam@cedar.test>. Send them the signed PDF and the signing record yourself.",
            ],
            "{warnings:?}"
        );
        assert_eq!(m.sent.borrow().len(), 1, "the first recipient was emailed");
        still_executed(&conn, id);
    }

    #[test]
    fn missing_mail_configuration_is_one_sentence_with_what_to_do() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let mailgun = Unavailable {
            service: "Mailgun",
            keys: &["mailgun_api_key", "mailgun_domain", "from_email"],
            state: UnavailableState::Partial(vec!["from_email"]),
        };
        let warnings = complete_execution::<_, _, FakeMailer>(
            &conn,
            dir.path(),
            id,
            "Initech",
            Ok(&p),
            Some(&s),
            Err(&mailgun),
        );
        let mail: Vec<_> = warnings.iter().filter(|w| w.contains("Mailgun")).collect();
        assert_eq!(
            mail,
            ["Warning: Mailgun is only partly configured (missing: from_email), so no signed copy was emailed. Send the signed PDF and the signing record to the recipients yourself, and set from_email to email them for future documents."]
        );
        still_executed(&conn, id);
    }

    #[test]
    fn a_version_with_no_recipients_says_nobody_was_emailed() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let version = latest_version(&conn, id).unwrap();
        conn.execute(
            "DELETE FROM document_recipients WHERE version_id = ?1",
            [version.id],
        )
        .unwrap();
        let m = FakeMailer::default();
        let warnings =
            complete_execution(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s), Ok(&m));
        assert!(
            warnings.contains(
                &"Warning: version 1 has no recipients on record, so no signed copy was emailed."
                    .to_string()
            ),
            "{warnings:?}"
        );
        assert!(m.sent.borrow().is_empty());
    }

    #[test]
    fn with_nothing_configured_the_countersignature_stands() {
        let (dir, conn) = test_conn();
        let (id, _, _) = executed(&conn, dir.path());
        let (r2, mailgun) = (unset("R2"), unset("Mailgun"));
        let warnings = complete_execution::<FakeDocumentPublisher, FakeResponseSource, FakeMailer>(
            &conn,
            dir.path(),
            id,
            "",
            Err(&r2),
            None,
            Err(&mailgun),
        );
        assert!(!warnings.is_empty());
        if cfg!(feature = "pdf") {
            assert!(dir.path().join(signing_record_rel(id, 1)).exists());
        } else {
            assert!(warnings.iter().any(|w| w == RECORD_UNAVAILABLE));
        }
        still_executed(&conn, id);
    }

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn without_pdf_support_the_signed_copy_still_goes_out_alone() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let m = FakeMailer::default();
        let warnings =
            complete_execution(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s), Ok(&m));
        assert_eq!(warnings, vec![RECORD_UNAVAILABLE.to_string()]);
        assert!(
            RECORD_UNAVAILABLE.starts_with("Warning: ")
                && RECORD_UNAVAILABLE.contains("`pdf` feature")
        );
        let sent = m.sent.borrow();
        assert!(sent.iter().all(|mail| mail.attachments.len() == 1));
        assert!(
            regenerate_signing_record(&conn, dir.path(), id, "Initech", Ok(&p), Some(&s)).is_err()
        );
    }
}
