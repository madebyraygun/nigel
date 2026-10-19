//! What follows a countersignature. The countersignature is committed first;
//! the signing record, the republished pages and the confirmation email then
//! follow best-effort, in that order, and whatever could not be done comes back
//! as warnings. Nothing here undoes the countersignature.
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::lifecycle::republish_linking_record;
use super::render::{
    attachment_name, relative_signing_record_href, render_signed_email_text, signed_subject,
};
use super::signing_record::{
    generate_signing_record, signing_record_rel, GeneratedRecord, RECORD_UNAVAILABLE,
};
use super::store::{get_document, latest_sent_version, read_version_pdf, recipients};
use crate::error::{NigelError, Result};
use crate::invoicing::gateway::{
    Attachment, DocumentPublisher, Mailer, OutgoingMail, ResponseSource,
};
use crate::invoicing::mailgun::format_address;
use crate::invoicing::r2::document_page_url;

/// The command that regenerates and republishes a signing record.
fn retry_hint(id: i64) -> String {
    format!("run `nigel document signing-record {id}` to try again")
}

/// Generate the signing record, publish it, then republish the pages linking
/// it and email every recipient of the executed version a signed copy.
pub fn complete_execution<P: DocumentPublisher, R: ResponseSource, M: Mailer>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
    mailer: Option<&M>,
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
                "Warning: the signing record could not be generated ({e}); {}.",
                retry_hint(id)
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
        publisher,
        mailer,
        &mut warnings,
    );
    warnings
}

/// A regenerated record: where its local copy is, and what could not be
/// published.
#[derive(Debug)]
pub struct RegeneratedRecord {
    pub path: PathBuf,
    pub filename: String,
    pub warnings: Vec<String>,
}

/// Regenerate an executed document's signing record, write it locally, and
/// republish it and the pages linking it when a publisher is configured.
/// Refused for a document that is not executed or a build without PDF
/// support; nothing is emailed.
pub fn regenerate_signing_record<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Result<RegeneratedRecord> {
    let record = generate_signing_record(conn, data_dir, id, company)?
        .ok_or_else(|| NigelError::Invalid(RECORD_UNAVAILABLE.to_string()))?;
    let mut warnings = Vec::new();
    publish_record_and_pages(
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
        warnings,
    })
}

/// Publish the record, then the pages: a page links the record only once the
/// record is up, so no page ever points at an object that is not there.
fn publish_record_and_pages<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    id: i64,
    company: &str,
    record: Option<&GeneratedRecord>,
    publisher: Option<&P>,
    source: Option<&R>,
    warnings: &mut Vec<String>,
) {
    let mut published = false;
    if let (Some(record), Some(publisher)) = (record, publisher) {
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
                "Warning: the countersignature is recorded, but the signing record could not be published ({e}); {}.",
                retry_hint(id)
            )),
        }
    }
    let href = record.map(|r| relative_signing_record_href(r.version));
    warnings.extend(republish_linking_record(
        conn,
        id,
        company,
        href.as_deref().filter(|_| published),
        publisher,
        source,
    ));
}

/// Email each recipient of the executed version, one message apiece, with the
/// signed PDF and the signing record attached.
#[allow(clippy::too_many_arguments)]
fn email_signed_copies<P: DocumentPublisher, M: Mailer>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    company: &str,
    record: Option<&GeneratedRecord>,
    publisher: Option<&P>,
    mailer: Option<&M>,
    warnings: &mut Vec<String>,
) {
    let Some(mailer) = mailer else {
        warnings.push(
            "Warning: Mailgun is not configured, so no signed copy was emailed to the recipients."
                .to_string(),
        );
        return;
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
                "Warning: no signed copy was emailed, the document could not be loaded ({e})."
            ));
            return;
        }
    };
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
    let subject = signed_subject(company, &document.title);
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
                "Warning: the signed copy could not be emailed to {} ({e}).",
                r.name
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::DocumentStatus;
    use crate::documents::record::{record_countersign, record_manual_accept};
    use crate::documents::store::{latest_version, signatures};
    use crate::documents::testing::{
        sent_document_with_fakes, test_conn, FakeDocumentPublisher, FakeMailer, FakeResponseSource,
    };
    #[cfg(feature = "pdf")]
    use crate::invoicing::r2::document_signing_record_key;

    fn executed(conn: &Connection, dir: &Path) -> (i64, FakeDocumentPublisher, FakeResponseSource) {
        let (id, p, s) = sent_document_with_fakes(conn, dir);
        record_manual_accept(conn, id, "Pat Example", "2026-10-06").unwrap();
        record_countersign(conn, id, "Sam Example", "2026-10-07").unwrap();
        (id, p, s)
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

        let warnings = complete_execution(
            &conn,
            dir.path(),
            id,
            "Initech",
            Some(&p),
            Some(&s),
            Some(&m),
        );
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
        assert!(!p
            .keys()
            .iter()
            .skip(before)
            .any(|k| k.ends_with("document.pdf")));
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
        complete_execution(
            &conn,
            dir.path(),
            id,
            "Initech",
            Some(&p),
            Some(&s),
            Some(&m),
        );
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
            Some(&failing),
            Some(&s),
            Some(&m),
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("signing record could not be published")
                && warnings[0].contains(&format!("nigel document signing-record {id}")),
            "{warnings:?}"
        );
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

        let p = FakeDocumentPublisher::default();
        let regenerated =
            regenerate_signing_record(&conn, dir.path(), id, "Initech", Some(&p), Some(&s))
                .unwrap();
        assert!(
            regenerated.warnings.is_empty(),
            "{:?}",
            regenerated.warnings
        );
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

    #[test]
    fn mail_failures_and_missing_configuration_are_warnings_and_the_document_stays_executed() {
        let (dir, conn) = test_conn();
        let (id, p, s) = executed(&conn, dir.path());
        let m = FakeMailer {
            fail_on_call: Some(1),
            ..Default::default()
        };
        let warnings = complete_execution(
            &conn,
            dir.path(),
            id,
            "Initech",
            Some(&p),
            Some(&s),
            Some(&m),
        );
        let mail_warnings: Vec<_> = warnings
            .iter()
            .filter(|w| w.contains("could not be emailed"))
            .collect();
        assert_eq!(mail_warnings.len(), 1, "{warnings:?}");
        assert!(mail_warnings[0].contains("Sam Example"));
        assert_eq!(m.sent.borrow().len(), 1, "the first recipient was emailed");
        still_executed(&conn, id);

        let warnings = complete_execution::<_, _, FakeMailer>(
            &conn,
            dir.path(),
            id,
            "Initech",
            Some(&p),
            Some(&s),
            None,
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("Mailgun is not configured")),
            "{warnings:?}"
        );
        still_executed(&conn, id);
    }

    #[test]
    fn with_nothing_configured_the_countersignature_stands() {
        let (dir, conn) = test_conn();
        let (id, _, _) = executed(&conn, dir.path());
        let warnings = complete_execution::<FakeDocumentPublisher, FakeResponseSource, FakeMailer>(
            &conn,
            dir.path(),
            id,
            "",
            None,
            None,
            None,
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
        let warnings = complete_execution(
            &conn,
            dir.path(),
            id,
            "Initech",
            Some(&p),
            Some(&s),
            Some(&m),
        );
        assert_eq!(warnings, vec![RECORD_UNAVAILABLE.to_string()]);
        let sent = m.sent.borrow();
        assert!(sent.iter().all(|mail| mail.attachments.len() == 1));
        assert!(
            regenerate_signing_record(&conn, dir.path(), id, "Initech", Some(&p), Some(&s))
                .is_err()
        );
    }
}
