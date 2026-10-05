//! The transitions that write rows: new versions, the frozen recipient set,
//! sending, manual and online responses, decline, countersign and withdraw.
//! Every one is refused by the guard table before it writes, and none writes a
//! status: the status is derived from the rows.
use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::documents::guards::{self, ensure_allowed, Action};
use crate::documents::model::{
    checksum_of, validate_moment, validate_note, validate_recipient, DocumentStatus, NewRecipient,
    Recipient, RecipientRole, ResponseKind,
};
use crate::documents::status::document_status;
use crate::documents::store::{
    ensure_pdf, get_document, latest_sent_version, latest_version, recipients, version_file_rel,
    write_version_file,
};
use crate::error::{NigelError, Result};
use crate::invoicing::invoices::validate_date;
use crate::invoicing::mailgun::validate_header_value;

fn touch(conn: &Connection, document_id: i64, date: &str) -> Result<()> {
    conn.execute(
        "UPDATE documents SET updated_at = ?2 WHERE id = ?1",
        params![document_id, &date[..10]],
    )?;
    Ok(())
}

pub fn add_version(
    conn: &Connection,
    data_dir: &Path,
    id: i64,
    pdf: &[u8],
    today: &str,
) -> Result<i64> {
    let document = get_document(conn, id)?;
    ensure_allowed(id, document.status, Action::Revise)?;
    guards::ensure_client_active_for_documents(conn, document.client_id)?;
    ensure_pdf(pdf)?;
    let today = validate_date(today, "revision date")?;
    let checksum = checksum_of(pdf);

    let tx = conn.unchecked_transaction()?;
    let earlier: Option<i64> = tx
        .query_row(
            "SELECT number FROM document_versions WHERE document_id = ?1 AND checksum = ?2
              ORDER BY number LIMIT 1",
            params![id, checksum],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(number) = earlier {
        return Err(NigelError::Conflict {
            code: "unchanged_revision",
            message: format!(
                "This PDF is identical to version {number} of document #{id}; a revision needs different content."
            ),
        });
    }
    let number = latest_version(&tx, id)?.number + 1;
    let rel = version_file_rel(id, number);
    tx.execute(
        "INSERT INTO document_versions (document_id, number, file_path, checksum, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, number, rel, checksum, today],
    )?;
    touch(&tx, id, &today)?;
    if let Err(e) = write_version_file(data_dir, &rel, pdf) {
        let _ = std::fs::remove_file(data_dir.join(&rel));
        return Err(e);
    }
    if let Err(e) = tx.commit() {
        let _ = std::fs::remove_file(data_dir.join(&rel));
        return Err(e.into());
    }
    Ok(number)
}

fn version_sent(version_id: i64) -> NigelError {
    NigelError::Conflict {
        code: "version_sent",
        message: format!(
            "Version {version_id} is already sent or superseded; its recipients are fixed."
        ),
    }
}

fn ensure_latest_unsent(conn: &Connection, version_id: i64) -> Result<i64> {
    let row: Option<(i64, Option<String>, bool)> = conn
        .query_row(
            "SELECT v.document_id, v.sent_at,
                    v.number = (SELECT MAX(number) FROM document_versions WHERE document_id = v.document_id)
               FROM document_versions v WHERE v.id = ?1",
            [version_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    match row {
        None => Err(NigelError::NotFound(format!(
            "Document version not found: id {version_id}"
        ))),
        Some((document_id, None, true)) => Ok(document_id),
        Some(_) => Err(version_sent(version_id)),
    }
}

pub fn freeze_recipients(
    conn: &Connection,
    version_id: i64,
    set: &[(NewRecipient, String)],
) -> Result<Vec<Recipient>> {
    ensure_latest_unsent(conn, version_id)?;
    let signers = set
        .iter()
        .filter(|(r, _)| r.role == RecipientRole::Signer)
        .count();
    if signers != 1 {
        return Err(NigelError::Conflict {
            code: "signer_count",
            message: format!(
                "A document is sent to exactly one signer; this send names {signers}."
            ),
        });
    }
    let mut seen = HashSet::new();
    for (r, _) in set {
        validate_recipient(r)?;
        if !seen.insert(r.email.trim().to_lowercase()) {
            return Err(NigelError::Invalid(format!(
                "{} is named twice; each address can appear once.",
                r.email
            )));
        }
    }
    let ordered = set
        .iter()
        .filter(|(r, _)| r.role == RecipientRole::Signer)
        .chain(set.iter().filter(|(r, _)| r.role != RecipientRole::Signer));

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM document_recipients WHERE version_id = ?1",
        [version_id],
    )?;
    for (position, (r, token)) in ordered.enumerate() {
        tx.execute(
            "INSERT INTO document_recipients (version_id, role, name, email, token, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                version_id,
                r.role.as_str(),
                r.name.trim(),
                r.email.trim(),
                token,
                position as i64
            ],
        )?;
    }
    tx.commit()?;
    recipients(conn, version_id)
}

pub fn unfreeze(conn: &Connection, version_id: i64) -> Result<()> {
    ensure_latest_unsent(conn, version_id)?;
    conn.execute(
        "DELETE FROM document_recipients WHERE version_id = ?1",
        [version_id],
    )?;
    Ok(())
}

pub fn mark_sent(conn: &Connection, version_id: i64, today: &str) -> Result<()> {
    let document_id = ensure_latest_unsent(conn, version_id)?;
    let today = validate_date(today, "send date")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE document_versions SET sent_at = ?2 WHERE id = ?1",
        params![version_id, today],
    )?;
    touch(&tx, document_id, &today)?;
    tx.commit()?;
    Ok(())
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NigelError::Invalid("A name is required.".into()));
    }
    validate_header_value(name, "name")?;
    Ok(name.to_string())
}

struct Bound {
    version_id: i64,
    checksum: String,
}

fn bind_manual(conn: &Connection, id: i64, action: Action) -> Result<Bound> {
    let status = document_status(conn, id)?;
    ensure_allowed(id, status, action)?;
    let version = latest_sent_version(conn, id)?.ok_or_else(|| NigelError::Conflict {
        code: "document_wrong_state",
        message: format!("Document #{id} has no sent version to record against."),
    })?;
    Ok(Bound {
        version_id: version.id,
        checksum: version.checksum,
    })
}

fn record_manual_signature(
    conn: &Connection,
    id: i64,
    name: &str,
    date: &str,
    action: Action,
    role: &str,
) -> Result<()> {
    let name = clean_name(name)?;
    let at = validate_moment(date, "response date")?;
    let bound = bind_manual(conn, id, action)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO document_signatures (version_id, recipient_id, role, name, method, signed_at, checksum)
         VALUES (?1, NULL, ?2, ?3, 'manual', ?4, ?5)",
        params![bound.version_id, role, name, at, bound.checksum],
    )?;
    touch(&tx, id, &at)?;
    tx.commit()?;
    Ok(())
}

pub fn record_manual_accept(conn: &Connection, id: i64, name: &str, date: &str) -> Result<()> {
    record_manual_signature(conn, id, name, date, Action::Accept, "client")
}

pub fn record_countersign(conn: &Connection, id: i64, name: &str, date: &str) -> Result<()> {
    record_manual_signature(conn, id, name, date, Action::Countersign, "countersign")
}

pub fn record_manual_change_request(
    conn: &Connection,
    id: i64,
    name: &str,
    note: &str,
    date: &str,
) -> Result<()> {
    let name = clean_name(name)?;
    let note = validate_note(note)?;
    let at = validate_moment(date, "response date")?;
    let bound = bind_manual(conn, id, Action::RequestChanges)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO document_change_requests (version_id, recipient_id, name, method, requested_at, note, checksum)
         VALUES (?1, NULL, ?2, 'manual', ?3, ?4, ?5)",
        params![bound.version_id, name, at, note, bound.checksum],
    )?;
    touch(&tx, id, &at)?;
    tx.commit()?;
    Ok(())
}

pub fn record_decline(conn: &Connection, id: i64, note: Option<&str>, date: &str) -> Result<()> {
    let note = note.map(validate_note).transpose()?;
    let at = validate_moment(date, "decline date")?;
    ensure_allowed(id, document_status(conn, id)?, Action::Decline)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE documents SET declined_at = ?2, decline_note = ?3 WHERE id = ?1",
        params![id, at, note],
    )?;
    touch(&tx, id, &at)?;
    tx.commit()?;
    Ok(())
}

pub fn record_withdrawal(conn: &Connection, id: i64, date: &str) -> Result<()> {
    let at = validate_moment(date, "withdrawal date")?;
    ensure_allowed(id, document_status(conn, id)?, Action::Withdraw)?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE documents SET withdrawn_at = ?2 WHERE id = ?1",
        params![id, at],
    )?;
    touch(&tx, id, &at)?;
    tx.commit()?;
    Ok(())
}

pub struct OnlineResponse<'a> {
    pub version_id: i64,
    pub recipient_id: i64,
    pub kind: ResponseKind<'a>,
    pub received_at: &'a str,
    pub ip: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub checksum: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    Recorded,
    AlreadyRecorded,
}

/// The published form takes responses only while the version is `sent`. A
/// change request closes it, so an acceptance that arrives in the same sync
/// window is refused rather than recorded over the outstanding request; the
/// manual verbs keep the wider guard table.
fn ensure_open_online(document_id: i64, status: DocumentStatus, action: Action) -> Result<()> {
    ensure_allowed(document_id, status, action)?;
    if status != DocumentStatus::Sent {
        return Err(NigelError::Conflict {
            code: "document_wrong_state",
            message: format!(
                "Document #{document_id} is {} and takes no more online responses on this version.",
                status.as_str()
            ),
        });
    }
    Ok(())
}

pub fn record_online_response(conn: &Connection, r: &OnlineResponse<'_>) -> Result<RecordOutcome> {
    let tx = conn.unchecked_transaction()?;
    let (document_id, role, name, email): (i64, String, String, String) = tx
        .query_row(
            "SELECT v.document_id, rc.role, rc.name, rc.email
               FROM document_recipients rc JOIN document_versions v ON v.id = rc.version_id
              WHERE rc.id = ?1 AND rc.version_id = ?2",
            params![r.recipient_id, r.version_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| {
            NigelError::Invalid(format!(
                "Recipient {} is not on version {}.",
                r.recipient_id, r.version_id
            ))
        })?;
    let already: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM document_signatures WHERE version_id = ?1 AND recipient_id = ?2)
             OR EXISTS(SELECT 1 FROM document_change_requests WHERE version_id = ?1 AND recipient_id = ?2)",
        params![r.version_id, r.recipient_id],
        |row| row.get(0),
    )?;
    if already {
        return Ok(RecordOutcome::AlreadyRecorded);
    }
    let version = match latest_sent_version(&tx, document_id)? {
        Some(v) if v.id == r.version_id => v,
        _ => {
            return Err(NigelError::Conflict {
                code: "stale_version",
                message: format!(
                    "This response is for a version of document #{document_id} that is no longer the latest sent."
                ),
            })
        }
    };
    if r.checksum != version.checksum {
        return Err(NigelError::Conflict {
            code: "checksum_mismatch",
            message: format!(
                "The response was given on {} but version {} is {}.",
                r.checksum, version.number, version.checksum
            ),
        });
    }
    let status = document_status(&tx, document_id)?;
    let at = validate_moment(r.received_at, "response")?;
    match r.kind {
        ResponseKind::Accept { typed_name } => {
            if role != RecipientRole::Signer.as_str() {
                return Err(NigelError::Conflict {
                    code: "role_not_allowed",
                    message: format!(
                        "{name} is a collaborator on document #{document_id} and cannot accept it."
                    ),
                });
            }
            ensure_open_online(document_id, status, Action::Accept)?;
            tx.execute(
                "INSERT INTO document_signatures (version_id, recipient_id, role, name, email, method, signed_at, typed_name, ip, user_agent, checksum)
                 VALUES (?1, ?2, 'client', ?3, ?4, 'online', ?5, ?6, ?7, ?8, ?9)",
                params![r.version_id, r.recipient_id, name, email, at, typed_name, r.ip, r.user_agent, r.checksum],
            )?;
        }
        ResponseKind::RequestChanges { note } => {
            ensure_open_online(document_id, status, Action::RequestChanges)?;
            let note = validate_note(note)?;
            tx.execute(
                "INSERT INTO document_change_requests (version_id, recipient_id, name, email, method, requested_at, note, ip, user_agent, checksum)
                 VALUES (?1, ?2, ?3, ?4, 'online', ?5, ?6, ?7, ?8, ?9)",
                params![r.version_id, r.recipient_id, name, email, at, note, r.ip, r.user_agent, r.checksum],
            )?;
        }
    }
    touch(&tx, document_id, &at)?;
    tx.commit()?;
    Ok(RecordOutcome::Recorded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::{gen_document_token, Method, SignatureRole};
    use crate::documents::status::document_status;
    use crate::documents::store::{change_requests, get_document, latest_version, signatures};
    use crate::documents::testing::{fixture_pdf, seed_client, seed_document, test_conn};

    fn sent_document(conn: &Connection, dir: &Path) -> (i64, i64, i64, i64) {
        let id = seed_document(conn, dir, seed_client(conn, "Cedar Systems"), "SOW");
        let v = latest_version(conn, id).unwrap();
        let recipients = freeze_recipients(
            conn,
            v.id,
            &[
                (
                    NewRecipient {
                        role: RecipientRole::Signer,
                        name: "Pat Example".into(),
                        email: "pat@cedar.test".into(),
                    },
                    gen_document_token(),
                ),
                (
                    NewRecipient {
                        role: RecipientRole::Collaborator,
                        name: "Sam Example".into(),
                        email: "sam@cedar.test".into(),
                    },
                    gen_document_token(),
                ),
            ],
        )
        .unwrap();
        mark_sent(conn, v.id, "2026-10-02").unwrap();
        (id, v.id, recipients[0].id, recipients[1].id)
    }

    #[test]
    fn freezing_refuses_an_unnamed_recipient_or_a_malformed_address_and_writes_nothing() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "SOW",
        );
        let v = latest_version(&conn, id).unwrap();
        let one = |role, name: &str, email: &str| {
            (
                NewRecipient {
                    role,
                    name: name.into(),
                    email: email.into(),
                },
                gen_document_token(),
            )
        };
        let signer = || one(RecipientRole::Signer, "Pat Example", "pat@cedar.test");

        let err = freeze_recipients(
            &conn,
            v.id,
            &[one(RecipientRole::Signer, "  ", "pat@cedar.test")],
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                NigelError::Conflict {
                    code: "signer_name_required",
                    ..
                }
            ),
            "{err:?}"
        );
        let err = freeze_recipients(
            &conn,
            v.id,
            &[
                signer(),
                one(RecipientRole::Collaborator, "", "sam@cedar.test"),
            ],
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)), "{err:?}");
        for bad in [
            "not-an-address",
            "@cedar.test",
            "sam@",
            "sam@cedar",
            "sam @cedar.test",
        ] {
            let err = freeze_recipients(
                &conn,
                v.id,
                &[
                    signer(),
                    one(RecipientRole::Collaborator, "Sam Example", bad),
                ],
            )
            .unwrap_err();
            assert!(
                matches!(&err, NigelError::Invalid(m) if m.contains(bad.trim())),
                "{bad}: {err:?}"
            );
        }
        assert!(recipients(&conn, v.id).unwrap().is_empty());
    }

    #[test]
    fn a_send_needs_exactly_one_signer() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "SOW",
        );
        let v = latest_version(&conn, id).unwrap();
        let collab = |e: &str| {
            (
                NewRecipient {
                    role: RecipientRole::Collaborator,
                    name: e.into(),
                    email: e.into(),
                },
                gen_document_token(),
            )
        };
        let signer = |e: &str| {
            (
                NewRecipient {
                    role: RecipientRole::Signer,
                    name: "Pat Example".into(),
                    email: e.into(),
                },
                gen_document_token(),
            )
        };
        for set in [
            vec![collab("sam@cedar.test")],
            vec![signer("a@cedar.test"), signer("b@cedar.test")],
        ] {
            let err = freeze_recipients(&conn, v.id, &set).unwrap_err();
            assert!(matches!(
                err,
                NigelError::Conflict {
                    code: "signer_count",
                    ..
                }
            ));
        }
        assert_eq!(
            freeze_recipients(
                &conn,
                v.id,
                &[signer("pat@cedar.test"), collab("sam@cedar.test")]
            )
            .unwrap()
            .len(),
            2
        );
    }

    #[test]
    fn the_signer_leads_the_frozen_order_and_a_refreeze_replaces_the_set() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "SOW",
        );
        let v = latest_version(&conn, id).unwrap();
        let new = |role, name: &str, email: &str| {
            (
                NewRecipient {
                    role,
                    name: name.into(),
                    email: email.into(),
                },
                gen_document_token(),
            )
        };
        let frozen = freeze_recipients(
            &conn,
            v.id,
            &[
                new(RecipientRole::Collaborator, "Sam Example", "sam@cedar.test"),
                new(RecipientRole::Signer, "Pat Example", "pat@cedar.test"),
            ],
        )
        .unwrap();
        assert_eq!(
            frozen
                .iter()
                .map(|r| (r.position, r.role))
                .collect::<Vec<_>>(),
            vec![(0, RecipientRole::Signer), (1, RecipientRole::Collaborator)]
        );
        let again = freeze_recipients(
            &conn,
            v.id,
            &[new(RecipientRole::Signer, "Pat Example", "pat@cedar.test")],
        )
        .unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(
            crate::documents::store::recipients(&conn, v.id)
                .unwrap()
                .len(),
            1
        );
        unfreeze(&conn, v.id).unwrap();
        assert!(crate::documents::store::recipients(&conn, v.id)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_duplicate_address_is_refused_ignoring_case() {
        let (dir, conn) = test_conn();
        let id = seed_document(
            &conn,
            dir.path(),
            seed_client(&conn, "Cedar Systems"),
            "SOW",
        );
        let v = latest_version(&conn, id).unwrap();
        let r = |role, e: &str| {
            (
                NewRecipient {
                    role,
                    name: "Pat Example".into(),
                    email: e.into(),
                },
                gen_document_token(),
            )
        };
        let err = freeze_recipients(
            &conn,
            v.id,
            &[
                r(RecipientRole::Signer, "pat@cedar.test"),
                r(RecipientRole::Collaborator, "PAT@Cedar.test"),
            ],
        )
        .unwrap_err();
        assert!(matches!(err, NigelError::Invalid(_)));
    }

    #[test]
    fn a_sent_version_keeps_its_recipients() {
        let (dir, conn) = test_conn();
        let (_, v, _, _) = sent_document(&conn, dir.path());
        assert!(matches!(
            unfreeze(&conn, v),
            Err(NigelError::Conflict {
                code: "version_sent",
                ..
            })
        ));
    }

    #[test]
    fn a_second_response_for_one_recipient_and_version_is_a_no_op_across_both_tables() {
        let (dir, conn) = test_conn();
        let (id, v, signer, _) = sent_document(&conn, dir.path());
        let checksum = latest_version(&conn, id).unwrap().checksum;
        let first = OnlineResponse {
            version_id: v,
            recipient_id: signer,
            kind: ResponseKind::RequestChanges {
                note: "Fix the dates",
            },
            received_at: "2026-10-05T17:04:11Z",
            ip: Some("203.0.113.7"),
            user_agent: Some("UA"),
            checksum: &checksum,
        };
        assert_eq!(
            record_online_response(&conn, &first).unwrap(),
            RecordOutcome::Recorded
        );
        let accept = OnlineResponse {
            kind: ResponseKind::Accept {
                typed_name: "Pat Example",
            },
            ..first
        };
        assert_eq!(
            record_online_response(&conn, &accept).unwrap(),
            RecordOutcome::AlreadyRecorded
        );
        assert!(signatures(&conn, v).unwrap().is_empty());
    }

    #[test]
    fn a_response_must_carry_the_checksum_it_was_shown() {
        let (dir, conn) = test_conn();
        let (_, v, signer, _) = sent_document(&conn, dir.path());
        let r = OnlineResponse {
            version_id: v,
            recipient_id: signer,
            kind: ResponseKind::Accept {
                typed_name: "Pat Example",
            },
            received_at: "2026-10-05T17:04:11Z",
            ip: None,
            user_agent: None,
            checksum: "sha256:00",
        };
        assert!(matches!(
            record_online_response(&conn, &r),
            Err(NigelError::Conflict {
                code: "checksum_mismatch",
                ..
            })
        ));
    }

    #[test]
    fn a_collaborator_cannot_accept() {
        let (dir, conn) = test_conn();
        let (id, v, _, collab) = sent_document(&conn, dir.path());
        let checksum = latest_version(&conn, id).unwrap().checksum;
        let r = OnlineResponse {
            version_id: v,
            recipient_id: collab,
            kind: ResponseKind::Accept {
                typed_name: "Sam Example",
            },
            received_at: "2026-10-05T17:04:11Z",
            ip: None,
            user_agent: None,
            checksum: &checksum,
        };
        assert!(matches!(
            record_online_response(&conn, &r),
            Err(NigelError::Conflict {
                code: "role_not_allowed",
                ..
            })
        ));
    }

    #[test]
    fn an_online_accept_stores_the_typed_name_and_the_recipients_own() {
        let (dir, conn) = test_conn();
        let (id, v, signer, _) = sent_document(&conn, dir.path());
        let checksum = latest_version(&conn, id).unwrap().checksum;
        let r = OnlineResponse {
            version_id: v,
            recipient_id: signer,
            kind: ResponseKind::Accept {
                typed_name: "P. Example",
            },
            received_at: "2026-10-05T17:04:11+00:00",
            ip: Some("203.0.113.7"),
            user_agent: Some("UA"),
            checksum: &checksum,
        };
        assert_eq!(
            record_online_response(&conn, &r).unwrap(),
            RecordOutcome::Recorded
        );
        let sig = &signatures(&conn, v).unwrap()[0];
        assert_eq!(sig.method, Method::Online);
        assert_eq!(sig.name, "Pat Example");
        assert_eq!(sig.typed_name.as_deref(), Some("P. Example"));
        assert_eq!(sig.signed_at, "2026-10-05T17:04:11Z");
        assert_eq!(sig.ip.as_deref(), Some("203.0.113.7"));
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Accepted
        );
    }

    #[test]
    fn a_response_to_a_superseded_version_is_stale() {
        let (dir, conn) = test_conn();
        let (id, v, signer, _) = sent_document(&conn, dir.path());
        let checksum = latest_version(&conn, id).unwrap().checksum;
        record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06")
            .unwrap();
        add_version(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07").unwrap();
        let v2 = latest_version(&conn, id).unwrap();
        freeze_recipients(
            &conn,
            v2.id,
            &[(
                NewRecipient {
                    role: RecipientRole::Signer,
                    name: "Pat Example".into(),
                    email: "pat@cedar.test".into(),
                },
                gen_document_token(),
            )],
        )
        .unwrap();
        mark_sent(&conn, v2.id, "2026-10-08").unwrap();
        let r = OnlineResponse {
            version_id: v,
            recipient_id: signer,
            kind: ResponseKind::Accept {
                typed_name: "Pat Example",
            },
            received_at: "2026-10-09T10:00:00Z",
            ip: None,
            user_agent: None,
            checksum: &checksum,
        };
        assert!(matches!(
            record_online_response(&conn, &r),
            Err(NigelError::Conflict {
                code: "stale_version",
                ..
            })
        ));
    }

    #[test]
    fn manual_responses_bind_to_the_latest_sent_version_and_carry_no_evidence() {
        let (dir, conn) = test_conn();
        let (id, v, _, _) = sent_document(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        let sig = &signatures(&conn, v).unwrap()[0];
        assert_eq!(
            (sig.method, sig.recipient_id, sig.ip.as_deref()),
            (Method::Manual, None, None)
        );
        assert_eq!(sig.checksum, latest_version(&conn, id).unwrap().checksum);
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Accepted
        );
        record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Executed
        );
        assert!(matches!(
            record_withdrawal(&conn, id, "2026-10-08"),
            Err(NigelError::Conflict {
                code: "document_terminal",
                ..
            })
        ));
    }

    #[test]
    fn an_accepted_document_with_an_unsent_next_version_reads_as_draft() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        assert_eq!(
            document_status(&conn, id).unwrap(),
            DocumentStatus::Accepted
        );
        conn.execute(
            "INSERT INTO document_versions (document_id, number, file_path, checksum, created_at)
             VALUES (?1, 2, 'documents/x/v2.pdf', 'sha256:02', '2026-10-07')",
            [id],
        )
        .unwrap();
        assert_eq!(document_status(&conn, id).unwrap(), DocumentStatus::Draft);
    }

    #[test]
    fn countersigning_an_accepted_document_derives_executed_from_the_rows() {
        let (dir, conn) = test_conn();
        let (id, v, _, _) = sent_document(&conn, dir.path());
        record_manual_accept(&conn, id, "Pat Example", "2026-10-06").unwrap();
        record_countersign(&conn, id, "Sam Example", "2026-10-07").unwrap();
        let roles: Vec<_> = signatures(&conn, v)
            .unwrap()
            .iter()
            .map(|s| s.role)
            .collect();
        assert_eq!(
            roles,
            vec![SignatureRole::Client, SignatureRole::Countersign]
        );
        assert_eq!(
            document_status(&conn, id).unwrap(),
            DocumentStatus::Executed
        );
    }

    #[test]
    fn countersigning_before_acceptance_is_refused() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        assert!(matches!(
            record_countersign(&conn, id, "Sam Example", "2026-10-07"),
            Err(NigelError::Conflict {
                code: "document_wrong_state",
                ..
            })
        ));
    }

    #[test]
    fn a_blank_name_is_refused() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        assert!(matches!(
            record_manual_accept(&conn, id, "  ", "2026-10-06"),
            Err(NigelError::Invalid(_))
        ));
    }

    #[test]
    fn withdrawing_a_sent_document_reads_as_withdrawn() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        record_withdrawal(&conn, id, "2026-10-06").unwrap();
        let doc = get_document(&conn, id).unwrap();
        assert_eq!(doc.status, DocumentStatus::Withdrawn);
        assert_eq!(doc.withdrawn_at.as_deref(), Some("2026-10-06"));
    }

    #[test]
    fn revise_after_a_change_request_makes_a_new_draft_version() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        record_manual_change_request(&conn, id, "Sam Example", "Fix the dates", "2026-10-06")
            .unwrap();
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::ChangesRequested
        );
        assert_eq!(
            add_version(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07").unwrap(),
            2
        );
        assert_eq!(
            get_document(&conn, id).unwrap().status,
            DocumentStatus::Draft
        );
        assert!(matches!(
            add_version(&conn, dir.path(), id, &fixture_pdf("v3"), "2026-10-07"),
            Err(NigelError::Conflict {
                code: "document_wrong_state",
                ..
            })
        ));
    }

    #[test]
    fn a_revision_with_the_same_bytes_is_refused() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        let same = std::fs::read(
            dir.path()
                .join(latest_version(&conn, id).unwrap().file_path),
        )
        .unwrap();
        assert!(matches!(
            add_version(&conn, dir.path(), id, &same, "2026-10-07"),
            Err(NigelError::Conflict {
                code: "unchanged_revision",
                ..
            })
        ));
    }

    #[test]
    fn a_revision_for_an_archived_client_is_refused() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        let client_id = get_document(&conn, id).unwrap().client_id;
        crate::invoicing::clients::archive_client(&conn, client_id, "2026-10-03").unwrap();
        assert!(matches!(
            add_version(&conn, dir.path(), id, &fixture_pdf("v2"), "2026-10-07"),
            Err(NigelError::Conflict {
                code: "client_archived",
                ..
            })
        ));
    }

    #[test]
    fn a_note_with_markup_is_stored_verbatim() {
        let (dir, conn) = test_conn();
        let (id, v, _, _) = sent_document(&conn, dir.path());
        let note = "<script>alert(1)</script> & <b>bold</b>";
        record_manual_change_request(&conn, id, "Sam Example", note, "2026-10-06").unwrap();
        assert_eq!(change_requests(&conn, v).unwrap()[0].note, note);
    }

    #[test]
    fn request_changes_is_refused_once_changes_were_requested() {
        let (dir, conn) = test_conn();
        let (id, _, _, _) = sent_document(&conn, dir.path());
        record_manual_change_request(&conn, id, "Sam Example", "One", "2026-10-06").unwrap();
        assert!(
            record_manual_change_request(&conn, id, "Sam Example", "Two", "2026-10-06").is_err()
        );
        record_decline(&conn, id, Some("Budget went elsewhere"), "2026-10-07").unwrap();
        let doc = get_document(&conn, id).unwrap();
        assert_eq!(doc.status, DocumentStatus::Declined);
        assert_eq!(doc.decline_note.as_deref(), Some("Budget went elsewhere"));
    }
}
