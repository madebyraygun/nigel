//! Pulls the responses the Worker wrote and records each one once.
use std::collections::HashSet;
use std::time::Instant;

use rusqlite::Connection;
use serde::Serialize;

use crate::documents::lifecycle::republish_after_change;
use crate::documents::model::{
    validate_moment, validate_note, DocumentListRow, DocumentStatus, DocumentVersion, Recipient,
    ResponseKind,
};
use crate::documents::record::{record_online_response, OnlineResponse, RecordOutcome};
use crate::documents::status::document_status;
use crate::documents::store::{
    change_requests, get_document, latest_sent_version, list_documents, recipients, signatures,
    DocumentFilter,
};
use crate::documents::wire::{DocumentResponse, ResponseAction};
use crate::error::{NigelError, Result};
use crate::invoicing::gateway::{DocumentPublisher, ResponseSource};

pub const DOCUMENT_BUDGET_EXHAUSTED: &str =
    "not checked: the sync time budget was used up before this document was reached";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSyncLine {
    pub document_id: i64,
    pub title: String,
    pub recorded: Vec<String>,
    pub refused: Vec<String>,
    pub warnings: Vec<String>,
    pub status: DocumentStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSyncFailure {
    pub document_id: i64,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSyncReport {
    pub documents_checked: u32,
    pub recorded: u32,
    pub lines: Vec<DocumentSyncLine>,
    pub failures: Vec<DocumentSyncFailure>,
}

fn reached_deadline(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

/// Checks every `sent` or `changes_requested` document, oldest change first.
/// A document whose responses could not be fetched is a failure and the run
/// moves on; the run is an error only when every document it reached failed.
pub fn sync_documents<R: ResponseSource, P: DocumentPublisher>(
    conn: &Connection,
    company: &str,
    source: &R,
    publisher: Option<&P>,
    deadline: Option<Instant>,
) -> Result<DocumentSyncReport> {
    let mut open: Vec<DocumentListRow> = list_documents(conn, &DocumentFilter::default())?
        .into_iter()
        .filter(|d| {
            matches!(
                d.status,
                DocumentStatus::Sent | DocumentStatus::ChangesRequested
            )
        })
        .collect();
    open.sort_by(|a, b| a.updated_at.cmp(&b.updated_at).then(a.id.cmp(&b.id)));

    let mut report = DocumentSyncReport::default();
    let mut reached_failures = 0;
    for (index, row) in open.iter().enumerate() {
        if reached_deadline(deadline) {
            report
                .failures
                .extend(open[index..].iter().map(|d| DocumentSyncFailure {
                    document_id: d.id,
                    message: DOCUMENT_BUDGET_EXHAUSTED.to_string(),
                }));
            break;
        }
        report.documents_checked += 1;
        match sync_document(conn, row.id, company, source, publisher) {
            Ok(line) => {
                report.recorded += line.recorded.len() as u32;
                report.lines.push(line);
            }
            Err(e) => {
                reached_failures += 1;
                report.failures.push(DocumentSyncFailure {
                    document_id: row.id,
                    message: e.to_string(),
                });
            }
        }
    }

    if report.documents_checked > 0 && reached_failures == report.documents_checked {
        let detail = report
            .failures
            .iter()
            .map(|f| format!("#{}: {}", f.document_id, f.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(NigelError::Other(format!(
            "all {} document(s) failed to sync — {detail}",
            report.documents_checked
        )));
    }
    Ok(report)
}

struct Fetched {
    recipient: Recipient,
    response: DocumentResponse,
}

fn sync_document<R: ResponseSource, P: DocumentPublisher>(
    conn: &Connection,
    id: i64,
    company: &str,
    source: &R,
    publisher: Option<&P>,
) -> Result<DocumentSyncLine> {
    let document = get_document(conn, id)?;
    let mut line = DocumentSyncLine {
        document_id: document.id,
        title: document.title.clone(),
        recorded: Vec::new(),
        refused: Vec::new(),
        warnings: Vec::new(),
        status: document.status,
    };
    let Some(version) = latest_sent_version(conn, document.id)? else {
        return Ok(line);
    };
    let answered: HashSet<i64> = signatures(conn, version.id)?
        .into_iter()
        .filter_map(|s| s.recipient_id)
        .chain(
            change_requests(conn, version.id)?
                .into_iter()
                .filter_map(|c| c.recipient_id),
        )
        .collect();

    let mut fetched = Vec::new();
    for recipient in recipients(conn, version.id)? {
        if answered.contains(&recipient.id) {
            continue;
        }
        if let Some(response) = source.fetch(&document.token, version.number, &recipient.token)? {
            fetched.push(Fetched {
                recipient,
                response,
            });
        }
    }

    let mut accepted = Vec::new();
    for f in fetched {
        match check(&f, &version) {
            Ok(checked) => accepted.push((checked, f)),
            Err(reason) => line.refused.push(reason),
        }
    }
    accepted.sort_by(|a, b| a.0.received_at.cmp(&b.0.received_at));

    for (checked, f) in &accepted {
        let r = &f.response;
        let kind = match r.action {
            ResponseAction::Accept => ResponseKind::Accept {
                typed_name: r.typed_name.as_deref().unwrap_or_default().trim(),
            },
            ResponseAction::RequestChanges => ResponseKind::RequestChanges {
                note: r.note.as_deref().unwrap_or_default(),
            },
        };
        let online = OnlineResponse {
            version_id: version.id,
            recipient_id: f.recipient.id,
            kind,
            received_at: &checked.received_at,
            ip: checked.ip.as_deref(),
            user_agent: checked.user_agent.as_deref(),
            checksum: &r.checksum,
        };
        match record_online_response(conn, &online) {
            Ok(RecordOutcome::Recorded) => line.recorded.push(match r.action {
                ResponseAction::Accept => {
                    format!("{} accepted version {}", f.recipient.name, version.number)
                }
                ResponseAction::RequestChanges => format!(
                    "{} requested changes on version {}",
                    f.recipient.name, version.number
                ),
            }),
            Ok(RecordOutcome::AlreadyRecorded) => {}
            Err(e @ (NigelError::Conflict { .. } | NigelError::Invalid(_))) => line.refused.push(
                format!("{}'s response was not recorded: {e}", f.recipient.name),
            ),
            Err(e) => return Err(e),
        }
    }

    line.status = document_status(conn, document.id)?;
    if line.status != document.status {
        line.warnings = republish_after_change(conn, document.id, company, publisher, Some(source));
    }
    Ok(line)
}

const TYPED_NAME_MAX_CHARS: usize = 200;
const USER_AGENT_MAX_CHARS: usize = 512;

/// What survives of a response's metadata once it is fit to store.
struct Checked {
    received_at: String,
    ip: Option<String>,
    user_agent: Option<String>,
}

/// Refuses a response that does not answer exactly this version for exactly
/// this recipient, or that carries a name or note unfit to record. An address
/// that is not an IP, or an over-long or control-laden user agent, is dropped
/// rather than refused. Nothing the response carries is echoed beyond what
/// gets recorded.
fn check(f: &Fetched, version: &DocumentVersion) -> std::result::Result<Checked, String> {
    let r = &f.response;
    let name = &f.recipient.name;
    let refuse = |why: String| Err(format!("{name}'s response was not recorded: {why}"));
    if r.version != version.number {
        return refuse(format!(
            "it answers version {} but version {} is the latest sent.",
            r.version, version.number
        ));
    }
    if r.checksum != version.checksum {
        return refuse(format!(
            "it was given on a different checksum than version {}.",
            version.number
        ));
    }
    if r.recipient_token != f.recipient.token {
        return refuse("it names a different recipient.".into());
    }
    match r.action {
        ResponseAction::Accept => {
            if r.consent != Some(true) {
                return refuse("an acceptance needs explicit consent.".into());
            }
            let typed = r.typed_name.as_deref().unwrap_or_default().trim();
            if typed.is_empty() {
                return refuse("an acceptance needs a typed name.".into());
            }
            if typed.chars().count() > TYPED_NAME_MAX_CHARS {
                return refuse(format!(
                    "the typed name is longer than {TYPED_NAME_MAX_CHARS} characters."
                ));
            }
            if typed.chars().any(char::is_control) {
                return refuse("the typed name carries control characters.".into());
            }
        }
        ResponseAction::RequestChanges => {
            let Some(note) = r.note.as_deref() else {
                return refuse("a change request needs a note.".into());
            };
            if let Err(e) = validate_note(note) {
                return refuse(e.to_string());
            }
        }
    }
    let received_at = validate_moment(&r.received_at, "response time")
        .map_err(|e| format!("{name}'s response was not recorded: {e}"))?;
    let ip =
        r.ip.as_deref()
            .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
            .map(|ip| ip.to_string());
    let user_agent = r
        .user_agent
        .as_deref()
        .filter(|ua| {
            ua.chars().count() <= USER_AGENT_MAX_CHARS && !ua.chars().any(char::is_control)
        })
        .map(str::to_string);
    Ok(Checked {
        received_at,
        ip,
        user_agent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::lifecycle::revise_with_republish;
    use crate::documents::model::{DocumentVersion, Method, Recipient};
    use crate::documents::record::record_manual_change_request;
    use crate::documents::send::{send_document, SendContext};
    use crate::documents::store::{
        change_requests, get_document, latest_version, recipients, signatures,
    };
    use crate::documents::testing::{
        fixture_pdf, pat, sam, sent_document_with_fakes, test_conn, FakeMailer, FakeResponseSource,
    };
    use crate::documents::wire::{DocumentResponse, ManifestState, ResponseAction};

    struct Sent {
        token: String,
        version: DocumentVersion,
        signer: Recipient,
        collaborator: Recipient,
    }

    fn sent_state(conn: &Connection, id: i64) -> Sent {
        let doc = get_document(conn, id).unwrap();
        let version = latest_version(conn, id).unwrap();
        let rs = recipients(conn, version.id).unwrap();
        Sent {
            token: doc.token,
            signer: rs[0].clone(),
            collaborator: rs[1].clone(),
            version,
        }
    }

    fn accept_from(s: &Sent, at: &str) -> DocumentResponse {
        DocumentResponse {
            action: ResponseAction::Accept,
            version: s.version.number,
            checksum: s.version.checksum.clone(),
            recipient_token: s.signer.token.clone(),
            typed_name: Some("Pat Example".into()),
            consent: Some(true),
            note: None,
            received_at: at.into(),
            ip: Some("203.0.113.7".into()),
            user_agent: Some("UA".into()),
        }
    }

    fn changes_from(s: &Sent, at: &str) -> DocumentResponse {
        DocumentResponse {
            action: ResponseAction::RequestChanges,
            recipient_token: s.collaborator.token.clone(),
            typed_name: None,
            consent: None,
            note: Some("Fix the dates".into()),
            ..accept_from(s, at)
        }
    }

    #[test]
    fn an_online_accept_is_recorded_once_and_rerunning_records_nothing() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            accept_from(&s, "2026-10-05T17:04:11Z"),
        );
        let first = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!((first.documents_checked, first.recorded), (1, 1));
        assert_eq!(first.lines[0].status, DocumentStatus::Accepted);
        assert_eq!(first.lines[0].recorded, ["Pat Example accepted version 1"]);
        let again = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!((again.documents_checked, again.recorded), (0, 0));
        let sigs = signatures(&conn, s.version.id).unwrap();
        assert_eq!(
            (sigs.len(), sigs[0].method, sigs[0].ip.as_deref()),
            (1, Method::Online, Some("203.0.113.7"))
        );
    }

    #[test]
    fn a_response_for_an_earlier_version_is_refused_and_records_nothing() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let v1 = sent_state(&conn, id);
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
            Some(&src),
        )
        .unwrap();
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
            &src,
        )
        .unwrap();
        let v2 = sent_state(&conn, id);
        src.put_response(
            &v1.token,
            1,
            &v1.signer.token,
            accept_from(&v1, "2026-10-08T10:00:00Z"),
        );
        src.put_response(
            &v2.token,
            2,
            &v2.signer.token,
            DocumentResponse {
                recipient_token: v2.signer.token.clone(),
                ..accept_from(&v1, "2026-10-08T10:01:00Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!(report.recorded, 0);
        assert_eq!(report.lines[0].refused.len(), 1);
        assert!(report.lines[0].refused[0].contains("version 1"));
        assert!(signatures(&conn, v2.version.id).unwrap().is_empty());
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Sent
        );
    }

    #[test]
    fn a_checksum_mismatch_is_refused() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            DocumentResponse {
                checksum: "sha256:00".into(),
                ..accept_from(&s, "2026-10-05T17:04:11Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!((report.recorded, report.lines[0].refused.len()), (0, 1));
    }

    #[test]
    fn two_responses_on_one_version_apply_in_received_order() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.collaborator.token,
            changes_from(&s, "2026-10-05T17:05:00Z"),
        );
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            accept_from(&s, "2026-10-05T17:00:00Z"),
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!(report.recorded, 1);
        assert_eq!(report.lines[0].refused.len(), 1);
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Accepted
        );
    }

    #[test]
    fn a_change_request_before_an_accept_records_both() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.collaborator.token,
            changes_from(&s, "2026-10-05T17:00:00Z"),
        );
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            accept_from(&s, "2026-10-05T17:05:00Z"),
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!(report.recorded, 2);
        assert_eq!(
            report.lines[0].recorded,
            [
                "Sam Example requested changes on version 1",
                "Pat Example accepted version 1"
            ]
        );
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Accepted
        );
    }

    #[test]
    fn an_accept_without_consent_is_refused() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            DocumentResponse {
                consent: None,
                ..accept_from(&s, "2026-10-05T17:04:11Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!((report.recorded, report.lines[0].refused.len()), (0, 1));
    }

    #[test]
    fn recipient_text_that_is_empty_or_carries_control_characters_is_refused() {
        let cases: [fn(&Sent) -> DocumentResponse; 6] = [
            |s| DocumentResponse {
                typed_name: Some("  ".into()),
                ..accept_from(s, "2026-10-05T17:04:11Z")
            },
            |s| DocumentResponse {
                typed_name: Some("Pat\u{1b}[2J Example".into()),
                ..accept_from(s, "2026-10-05T17:04:11Z")
            },
            |s| DocumentResponse {
                typed_name: None,
                ..accept_from(s, "2026-10-05T17:04:11Z")
            },
            |s| DocumentResponse {
                note: Some("Fix\u{7}the dates".into()),
                ..changes_from(s, "2026-10-05T17:04:11Z")
            },
            |s| DocumentResponse {
                note: None,
                ..changes_from(s, "2026-10-05T17:04:11Z")
            },
            |s| accept_from(s, "yesterday"),
        ];
        for case in cases {
            let (dir, conn) = test_conn();
            let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
            let s = sent_state(&conn, id);
            let response = case(&s);
            let rt = response.recipient_token.clone();
            src.put_response(&s.token, 1, &rt, response);
            let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
            assert_eq!((report.recorded, report.lines[0].refused.len()), (0, 1));
            assert!(signatures(&conn, s.version.id).unwrap().is_empty());
            assert!(change_requests(&conn, s.version.id).unwrap().is_empty());
        }
    }

    #[test]
    fn a_note_keeps_its_line_breaks() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.collaborator.token,
            DocumentResponse {
                note: Some("Fix the dates.\nAnd the totals.".into()),
                ..changes_from(&s, "2026-10-05T17:04:11Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!(report.recorded, 1);
        assert_eq!(
            change_requests(&conn, s.version.id).unwrap()[0].note,
            "Fix the dates.\nAnd the totals."
        );
    }

    #[test]
    fn an_over_long_typed_name_is_refused() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            DocumentResponse {
                typed_name: Some("P".repeat(201)),
                ..accept_from(&s, "2026-10-05T17:04:11Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!((report.recorded, report.lines[0].refused.len()), (0, 1));
        assert!(signatures(&conn, s.version.id).unwrap().is_empty());
    }

    #[test]
    fn an_unfit_user_agent_is_dropped_and_the_response_kept() {
        for agent in ["U".repeat(513), "UA\u{1b}[2J".to_string()] {
            let (dir, conn) = test_conn();
            let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
            let s = sent_state(&conn, id);
            src.put_response(
                &s.token,
                1,
                &s.signer.token,
                DocumentResponse {
                    user_agent: Some(agent),
                    ..accept_from(&s, "2026-10-05T17:04:11Z")
                },
            );
            let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
            assert_eq!(report.recorded, 1);
            let sigs = signatures(&conn, s.version.id).unwrap();
            assert_eq!(
                (sigs[0].user_agent.as_deref(), sigs[0].ip.as_deref()),
                (None, Some("203.0.113.7"))
            );
        }
    }

    #[test]
    fn a_non_ip_address_is_dropped_and_the_response_kept() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            DocumentResponse {
                ip: Some("203.0.113.7\n<script>".into()),
                ..accept_from(&s, "2026-10-05T17:04:11Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!(report.recorded, 1);
        let sigs = signatures(&conn, s.version.id).unwrap();
        assert_eq!(
            (sigs[0].ip.as_deref(), sigs[0].user_agent.as_deref()),
            (None, Some("UA"))
        );
    }

    #[test]
    fn a_hostile_checksum_is_never_echoed() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        let hostile = format!("sha256:\u{1b}]0;pwned\u{7}{}", "x".repeat(5000));
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            DocumentResponse {
                checksum: hostile,
                ..accept_from(&s, "2026-10-05T17:04:11Z")
            },
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        let refused = &report.lines[0].refused[0];
        assert!(refused.contains("a different checksum than version 1"));
        assert!(!refused.contains("pwned") && !refused.contains('\u{1b}'));
    }

    #[test]
    fn a_status_change_closes_the_manifest_and_republishes() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.collaborator.token,
            changes_from(&s, "2026-10-05T17:04:11Z"),
        );
        sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!(
            src.last_manifest(&s.token).unwrap().state,
            ManifestState::Closed
        );
        assert!(p
            .page(&s.token, &s.signer.token)
            .unwrap()
            .contains("Changes requested"));
    }

    #[test]
    fn a_failed_republish_is_a_warning() {
        let (dir, conn) = test_conn();
        let (id, p, src) = sent_document_with_fakes(&conn, dir.path());
        let s = sent_state(&conn, id);
        src.put_response(
            &s.token,
            1,
            &s.signer.token,
            accept_from(&s, "2026-10-05T17:04:11Z"),
        );
        let failing = FakeResponseSource {
            fail_put: true,
            responses: src.responses.clone(),
            ..Default::default()
        };
        let report = sync_documents(&conn, "Initech", &failing, Some(&p), None).unwrap();
        assert_eq!(report.recorded, 1);
        assert!(!report.lines[0].warnings.is_empty());
    }

    #[test]
    fn one_failing_fetch_does_not_stop_the_run() {
        let (dir, conn) = test_conn();
        let (first, p, _) = sent_document_with_fakes(&conn, dir.path());
        let (second, _, _) = sent_document_with_fakes(&conn, dir.path());
        let (a, b) = (sent_state(&conn, first), sent_state(&conn, second));
        let src = FakeResponseSource {
            fail_fetch_for: Some(a.token.clone()),
            ..Default::default()
        };
        src.put_response(
            &b.token,
            1,
            &b.signer.token,
            accept_from(&b, "2026-10-05T17:04:11Z"),
        );
        let report = sync_documents(&conn, "Initech", &src, Some(&p), None).unwrap();
        assert_eq!((report.documents_checked, report.recorded), (2, 1));
        assert_eq!(report.failures.len(), 1);
    }

    #[test]
    fn every_document_failing_is_an_err() {
        let (dir, conn) = test_conn();
        let (_, p, _) = sent_document_with_fakes(&conn, dir.path());
        sent_document_with_fakes(&conn, dir.path());
        let failing = FakeResponseSource {
            fail_fetch: true,
            ..Default::default()
        };
        assert!(sync_documents(&conn, "Initech", &failing, Some(&p), None).is_err());
    }

    #[test]
    fn an_empty_run_is_ok_with_zero_counts() {
        let (_dir, conn) = test_conn();
        let src = FakeResponseSource::default();
        let report = sync_documents(
            &conn,
            "Initech",
            &src,
            None::<&crate::documents::testing::FakeDocumentPublisher>,
            None,
        )
        .unwrap();
        assert_eq!((report.documents_checked, report.recorded), (0, 0));
    }

    #[test]
    fn a_spent_budget_reports_the_rest_as_failures() {
        let (dir, conn) = test_conn();
        let (_, p, src) = sent_document_with_fakes(&conn, dir.path());
        let report = sync_documents(
            &conn,
            "Initech",
            &src,
            Some(&p),
            Some(std::time::Instant::now()),
        )
        .unwrap();
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].message, DOCUMENT_BUDGET_EXHAUSTED);
    }
}
