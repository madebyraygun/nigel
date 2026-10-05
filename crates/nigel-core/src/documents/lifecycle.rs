//! Revise, withdraw and the republish after any change. The change is committed first; closing the response
//! manifest and rewriting the published pages follow best-effort, and whatever
//! could not be reached comes back as warnings.
use std::path::Path;

use rusqlite::Connection;

use super::model::{Document, DocumentStatus, DocumentVersion, SignatureRole};
use super::record::{add_version, record_withdrawal};
use super::render::{
    relative_pdf_href, render_recipient_page, withdrawn_page_html, PageContext, PageRecipient,
    PageState,
};
use super::store::{get_document, recipients, signatures, versions};
use super::wire::{manifest_for, ManifestState};
use crate::error::Result;
use crate::invoicing::clients::get_client;
use crate::invoicing::gateway::{DocumentPublisher, ResponseSource};

#[derive(Debug, Clone)]
pub struct ReviseOutcome {
    pub version: i64,
    pub warnings: Vec<String>,
}

fn latest_sent(conn: &Connection, id: i64) -> Result<Option<DocumentVersion>> {
    Ok(versions(conn, id)?
        .into_iter()
        .rev()
        .find(|v| v.sent_at.is_some()))
}

#[allow(clippy::too_many_arguments)]
pub fn revise_with_republish<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    pdf: &[u8],
    today: &str,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Result<ReviseOutcome> {
    let version = add_version(conn, data_dir, id, pdf, today)?;
    let document = get_document(conn, id)?;
    let warnings = match latest_sent(conn, id)? {
        Some(sent) => close_and_republish(
            conn,
            &document,
            &sent,
            company,
            &PageState::Revising,
            publisher,
            source,
        ),
        None => Vec::new(),
    };
    Ok(ReviseOutcome { version, warnings })
}

pub fn withdraw_with_teardown<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    id: i64,
    today: &str,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Result<Vec<String>> {
    record_withdrawal(conn, id, today)?;
    let document = get_document(conn, id)?;
    let sent: Vec<DocumentVersion> = versions(conn, id)?
        .into_iter()
        .filter(|v| v.sent_at.is_some())
        .collect();
    let newest = sent.len().saturating_sub(1);
    let mut warnings = Vec::new();
    for (i, version) in sent.iter().enumerate() {
        warnings.extend(teardown(
            conn,
            &document,
            version,
            i == newest,
            &|_| withdrawn_page_html(company, &document.title),
            publisher,
            source,
        ));
    }
    Ok(warnings)
}

type Stamp = (SignatureRole, String, String);
type Loaded = (Document, DocumentVersion, Vec<Stamp>);

pub fn republish_after_change<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    id: i64,
    company: &str,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Vec<String> {
    let load = || -> Result<Option<Loaded>> {
        let document = get_document(conn, id)?;
        let Some(version) = latest_sent(conn, id)? else {
            return Ok(None);
        };
        let stamps = signatures(conn, version.id)?
            .into_iter()
            .map(|s| (s.role, s.name, s.signed_at.chars().take(10).collect()))
            .collect();
        Ok(Some((document, version, stamps)))
    };
    let (document, version, stamps) = match load() {
        Ok(Some(loaded)) => loaded,
        Ok(None) => return Vec::new(),
        Err(e) => return vec![format!("Warning: could not load the document ({e}).")],
    };
    let stamp = |role: SignatureRole| {
        stamps
            .iter()
            .find(|(r, _, _)| *r == role)
            .map(|(_, name, date)| (name.as_str(), date.as_str()))
    };
    let state = match (document.status, stamp(SignatureRole::Client)) {
        (DocumentStatus::Accepted, Some((name, date))) => PageState::Accepted { name, date },
        (DocumentStatus::Executed, Some(client)) => match stamp(SignatureRole::Countersign) {
            Some(countersign) => PageState::Executed {
                client,
                countersign,
            },
            None => return Vec::new(),
        },
        (DocumentStatus::ChangesRequested, _) => PageState::ChangesRequested,
        (DocumentStatus::Declined, _) => PageState::Declined,
        _ => return Vec::new(),
    };
    close_and_republish(
        conn, &document, &version, company, &state, publisher, source,
    )
}

pub(crate) fn close_and_republish<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    document: &Document,
    version: &DocumentVersion,
    company: &str,
    state: &PageState<'_>,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Vec<String> {
    let client_name = match get_client(conn, document.client_id) {
        Ok(c) => c.name,
        Err(e) => return vec![format!("Warning: could not load the client ({e}).")],
    };
    let href = relative_pdf_href(version.number);
    let ctx = PageContext {
        company,
        client_name: &client_name,
        kind: &document.kind,
        title: &document.title,
        token: &document.token,
        version: version.number,
        checksum: &version.checksum,
        pdf_href: &href,
    };
    teardown(
        conn,
        document,
        version,
        true,
        &|recipient| render_recipient_page(&ctx, recipient, state),
        publisher,
        source,
    )
}

#[allow(clippy::too_many_arguments)]
fn teardown<P: DocumentPublisher, R: ResponseSource>(
    conn: &Connection,
    document: &Document,
    version: &DocumentVersion,
    close_manifest: bool,
    render: &dyn Fn(&PageRecipient<'_>) -> String,
    publisher: Option<&P>,
    source: Option<&R>,
) -> Vec<String> {
    let frozen = match recipients(conn, version.id) {
        Ok(r) if r.is_empty() => return Vec::new(),
        Ok(r) => r,
        Err(e) => return vec![format!("Warning: could not load the recipients ({e}).")],
    };
    let n = version.number;
    let mut warnings = Vec::new();

    if close_manifest {
        match source {
            None => warnings.push(format!(
                "Warning: r2_private_bucket is not configured, so version {n}'s response manifest is still open."
            )),
            Some(source) => {
                let manifest = manifest_for(version, &frozen, ManifestState::Closed);
                if let Err(e) = source.put_manifest(&document.token, &manifest) {
                    warnings.push(format!("Warning: could not close the response manifest ({e})."));
                }
            }
        }
    }

    match publisher {
        None => warnings.push(format!(
            "Warning: the change is recorded, but the R2 publisher is not configured, so the published pages for version {n} still show the old state."
        )),
        Some(publisher) => {
            for r in &frozen {
                let page = render(&PageRecipient {
                        token: &r.token,
                        role: r.role,
                        name: &r.name,
                    },
                );
                if let Err(e) = publisher.publish_page(&document.token, &r.token, page.as_bytes())
                {
                    warnings.push(format!(
                        "Warning: could not republish {}'s page ({e}).",
                        r.name
                    ));
                }
            }
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::{DocumentStatus, RecipientRole};
    use crate::documents::record::{
        record_countersign, record_manual_accept, record_manual_change_request,
    };
    use crate::documents::send::{send_document, SendContext};
    use crate::documents::store::{get_document, latest_version, recipients, signatures, versions};
    use crate::documents::testing::{
        fixture_pdf, pat, sam, seed_client, seed_document, sent_document_with_fakes, test_conn,
        FakeDocumentPublisher, FakeMailer, FakeResponseSource,
    };
    use crate::documents::wire::ManifestState;

    #[test]
    fn revise_closes_the_manifest_and_republishes_every_live_page_as_being_revised() {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06")
            .unwrap();
        let out = revise_with_republish(
            &conn,
            dir.path(),
            id,
            &fixture_pdf("v2"),
            "2026-10-07",
            "Initech",
            Some(&p),
            Some(&s),
        )
        .unwrap();
        assert_eq!((out.version, out.warnings.len()), (2, 0));
        let doc = get_document(&conn, id).unwrap();
        assert_eq!(
            s.last_manifest(&doc.token).unwrap().state,
            ManifestState::Closed
        );
        let v1 = &versions(&conn, id).unwrap()[0];
        for r in recipients(&conn, v1.id).unwrap() {
            let page = p.page(&doc.token, &r.token).unwrap();
            assert!(page.contains("being revised") && !page.contains("<form"));
        }
    }

    #[test]
    fn the_next_send_after_a_revise_reuses_the_document_token_with_new_recipient_tokens() {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06")
            .unwrap();
        revise_with_republish(
            &conn,
            dir.path(),
            id,
            &fixture_pdf("v2"),
            "2026-10-07",
            "Initech",
            Some(&p),
            Some(&s),
        )
        .unwrap();
        let token = get_document(&conn, id).unwrap().token;
        let ctx = SendContext {
            data_dir: dir.path(),
            company: "Initech",
            response_url: None,
            today: "2026-10-08",
        };
        send_document(
            &conn,
            id,
            &[pat(), sam()],
            &ctx,
            &p,
            &FakeMailer::default(),
            &s,
        )
        .unwrap();
        let all = versions(&conn, id).unwrap();
        let old: Vec<String> = recipients(&conn, all[0].id)
            .unwrap()
            .into_iter()
            .map(|r| r.token)
            .collect();
        let new: Vec<String> = recipients(&conn, all[1].id)
            .unwrap()
            .into_iter()
            .map(|r| r.token)
            .collect();
        assert!(new.iter().all(|t| !old.contains(t)));
        assert_eq!(get_document(&conn, id).unwrap().token, token);
        assert_eq!(s.last_manifest(&token).unwrap().version, 2);
    }

    #[test]
    fn withdraw_commits_first_and_reports_what_it_could_not_reach() {
        let (dir, conn) = test_conn();
        let (id, _, _) = sent_document_with_fakes(&conn, dir.path());
        let warnings = withdraw_with_teardown::<FakeDocumentPublisher, FakeResponseSource>(
            &conn,
            id,
            "2026-10-06",
            "Initech",
            None,
            None,
        )
        .unwrap();
        assert_eq!(warnings.len(), 2);
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Withdrawn
        );
    }

    #[test]
    fn withdrawing_a_sent_document_closes_the_manifest_and_replaces_every_page() {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        let warnings =
            withdraw_with_teardown(&conn, id, "2026-10-06", "Initech", Some(&p), Some(&s)).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let doc = get_document(&conn, id).unwrap();
        let manifest = s.last_manifest(&doc.token).unwrap();
        assert_eq!(
            (manifest.state, manifest.version),
            (ManifestState::Closed, 1)
        );
        let v1 = latest_version(&conn, id).unwrap();
        for r in recipients(&conn, v1.id).unwrap() {
            let page = p.page(&doc.token, &r.token).unwrap();
            assert!(
                page.contains("This document has been withdrawn.") && !page.contains("<form"),
                "{}",
                r.name
            );
        }
    }

    #[test]
    fn withdrawing_after_a_second_send_closes_the_newest_manifest_and_replaces_both_versions_pages()
    {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06")
            .unwrap();
        revise_with_republish(
            &conn,
            dir.path(),
            id,
            &fixture_pdf("v2"),
            "2026-10-07",
            "Initech",
            Some(&p),
            Some(&s),
        )
        .unwrap();
        let ctx = SendContext {
            data_dir: dir.path(),
            company: "Initech",
            response_url: Some("https://docs.example.test/d/respond"),
            today: "2026-10-08",
        };
        send_document(
            &conn,
            id,
            &[pat(), sam()],
            &ctx,
            &p,
            &FakeMailer::default(),
            &s,
        )
        .unwrap();
        let token = get_document(&conn, id).unwrap().token;
        let manifests_before = s.manifests.borrow().len();

        let warnings =
            withdraw_with_teardown(&conn, id, "2026-10-09", "Initech", Some(&p), Some(&s)).unwrap();

        assert!(warnings.is_empty(), "{warnings:?}");
        let written: Vec<_> = s.manifests.borrow()[manifests_before..].to_vec();
        assert_eq!(written.len(), 1, "the manifest is closed once");
        assert_eq!(
            (written[0].1.state, written[0].1.version),
            (ManifestState::Closed, 2)
        );
        let all = versions(&conn, id).unwrap();
        assert_eq!(all.len(), 2);
        for v in &all {
            for r in recipients(&conn, v.id).unwrap() {
                let page = p.page(&token, &r.token).unwrap();
                assert!(
                    page.contains("This document has been withdrawn.") && !page.contains("<form"),
                    "v{} {}",
                    v.number,
                    r.name
                );
            }
        }
    }

    #[test]
    fn a_partial_republish_failure_is_a_warning_not_an_error() {
        let (dir, conn) = test_conn();
        let (id, _, s) = sent_document_with_fakes(&conn, dir.path());
        let v = latest_version(&conn, id).unwrap();
        let sam_token = recipients(&conn, v.id)
            .unwrap()
            .into_iter()
            .find(|r| r.role == RecipientRole::Collaborator)
            .unwrap()
            .token;
        let p = FakeDocumentPublisher {
            fail_when_key_contains: Some(sam_token),
            ..Default::default()
        };
        let warnings =
            withdraw_with_teardown(&conn, id, "2026-10-06", "Initech", Some(&p), Some(&s)).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Sam Example"));
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Withdrawn
        );
    }

    #[test]
    fn withdrawing_a_draft_touches_nothing() {
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
        assert!(
            withdraw_with_teardown(&conn, id, "2026-10-06", "Initech", Some(&p), Some(&s))
                .unwrap()
                .is_empty()
        );
        assert!(p.keys().is_empty() && s.manifests.borrow().is_empty());
    }

    #[test]
    fn accepting_republishes_every_page_stamped_and_without_a_form() {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        assert!(republish_after_change(&conn, id, "Initech", Some(&p), Some(&s)).is_empty());
        let doc = get_document(&conn, id).unwrap();
        assert_eq!(
            s.last_manifest(&doc.token).unwrap().state,
            ManifestState::Closed
        );
        for r in recipients(&conn, latest_version(&conn, id).unwrap().id).unwrap() {
            let page = p.page(&doc.token, &r.token).unwrap();
            assert!(
                page.contains("Accepted by Pat Example on 2026-10-06") && !page.contains("<form")
            );
        }
    }

    #[test]
    fn countersigning_stamps_both_signatures() {
        let (dir, conn) = test_conn();
        let (id, p, s) = sent_document_with_fakes(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
        republish_after_change(&conn, id, "Initech", Some(&p), Some(&s));
        let doc = get_document(&conn, id).unwrap();
        let signer = &recipients(&conn, latest_version(&conn, id).unwrap().id).unwrap()[0];
        let page = p.page(&doc.token, &signer.token).unwrap();
        assert!(
            page.contains("Pat Example")
                && page.contains("Sam Example")
                && page.contains("2026-10-07")
        );
    }

    #[test]
    fn a_failed_republish_never_loses_the_signature() {
        let (dir, conn) = test_conn();
        let (id, _, s) = sent_document_with_fakes(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        let failing = FakeDocumentPublisher {
            fail_when_key_contains: Some("index.html".into()),
            ..Default::default()
        };
        assert_eq!(
            republish_after_change(&conn, id, "Initech", Some(&failing), Some(&s)).len(),
            2
        );
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Accepted
        );
        assert_eq!(
            signatures(&conn, latest_version(&conn, id).unwrap().id)
                .unwrap()
                .len(),
            1
        );
    }
}
