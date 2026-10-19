//! Filing a PDF as a document, editing a draft's title or kind, and every read
//! of the rows around it.
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::documents::guards::{self, Action};
use crate::documents::kinds::active_kind_by_name;
use crate::documents::model::{
    checksum_of, gen_document_token, ChangeRequest, Document, DocumentListRow, DocumentRecord,
    DocumentStatus, DocumentVersion, Method, Recipient, RecipientRole, Signature, SignatureRole,
    VersionRecord,
};
use crate::documents::status;
use crate::error::{NigelError, Result};
use crate::invoicing::invoices::validate_date;

pub const MIN_PDF_BYTES: usize = 8;
const TITLE_MAX_CHARS: usize = 200;

pub fn ensure_pdf(bytes: &[u8]) -> Result<()> {
    if bytes.len() >= MIN_PDF_BYTES && bytes.starts_with(b"%PDF-") {
        return Ok(());
    }
    Err(NigelError::Invalid(
        "This file is not a valid PDF, it may be damaged.".into(),
    ))
}

pub fn version_file_rel(document_id: i64, number: i64) -> String {
    format!("documents/{document_id}/v{number}.pdf")
}

pub fn write_version_file(data_dir: &Path, rel: &str, pdf: &[u8]) -> Result<()> {
    let path = data_dir.join(rel);
    let mut chain = Vec::new();
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == data_dir {
            break;
        }
        chain.push(d.to_path_buf());
        dir = d.parent();
    }
    std::fs::create_dir_all(path.parent().unwrap_or(data_dir))?;
    for d in chain.iter().rev() {
        crate::settings::restrict_dir_permissions(d)?;
    }
    std::fs::write(&path, pdf)?;
    crate::settings::restrict_file_permissions(&path)?;
    Ok(())
}

pub fn read_version_pdf(data_dir: &Path, version: &DocumentVersion) -> Result<Vec<u8>> {
    let bytes = std::fs::read(data_dir.join(&version.file_path))?;
    if checksum_of(&bytes) != version.checksum {
        return Err(NigelError::Conflict {
            code: "file_changed",
            message: format!(
                "The PDF for version {} of document #{} no longer matches the checksum recorded when it was filed.",
                version.number, version.document_id
            ),
        });
    }
    Ok(bytes)
}

pub struct NewDocument<'a> {
    pub client_id: i64,
    pub kind: &'a str,
    pub title: &'a str,
}

fn clean_title(title: &str) -> Result<String> {
    let title = title.trim();
    let count = title.chars().count();
    if count == 0 || count > TITLE_MAX_CHARS {
        return Err(NigelError::Invalid(format!(
            "A document title must be 1 to {TITLE_MAX_CHARS} characters, got {count}."
        )));
    }
    Ok(title.to_string())
}

/// A withdrawn or declined document no longer holds its PDF, so a send to the
/// wrong address can be withdrawn and the same file filed again.
fn ensure_not_filed(conn: &Connection, client_id: i64, checksum: &str) -> Result<()> {
    let existing: Option<(i64, String, String)> = conn
        .query_row(
            "SELECT d.id, d.title, c.name
               FROM document_versions v
               JOIN documents d ON d.id = v.document_id
               JOIN clients c ON c.id = d.client_id
              WHERE d.client_id = ?1 AND v.checksum = ?2
                AND d.withdrawn_at IS NULL AND d.declined_at IS NULL
              ORDER BY d.id LIMIT 1",
            params![client_id, checksum],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    match existing {
        None => Ok(()),
        Some((id, title, client)) => Err(NigelError::Conflict {
            code: "duplicate_document",
            message: format!(
                "This PDF is already filed for {client} as document #{id} (\"{title}\")."
            ),
        }),
    }
}

pub fn file_document(
    conn: &Connection,
    data_dir: &Path,
    new: &NewDocument<'_>,
    pdf: &[u8],
    today: &str,
) -> Result<i64> {
    guards::ensure_client_active_for_documents(conn, new.client_id)?;
    let kind = active_kind_by_name(conn, new.kind)?;
    let title = clean_title(new.title)?;
    ensure_pdf(pdf)?;
    let today = validate_date(today, "filing date")?;
    let checksum = checksum_of(pdf);

    let tx = conn.unchecked_transaction()?;
    ensure_not_filed(&tx, new.client_id, &checksum)?;
    tx.execute(
        "INSERT INTO documents (client_id, kind_id, title, token, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![new.client_id, kind.id, title, gen_document_token(), today],
    )?;
    let id = tx.last_insert_rowid();
    let rel = version_file_rel(id, 1);
    tx.execute(
        "INSERT INTO document_versions (document_id, number, file_path, checksum, created_at)
         VALUES (?1, 1, ?2, ?3, ?4)",
        params![id, rel, checksum, today],
    )?;
    if let Err(e) = write_version_file(data_dir, &rel, pdf) {
        let _ = std::fs::remove_file(data_dir.join(&rel));
        return Err(e);
    }
    if let Err(e) = tx.commit() {
        let _ = std::fs::remove_file(data_dir.join(&rel));
        return Err(e.into());
    }
    Ok(id)
}

const DOCUMENT_SELECT: &str =
    "SELECT d.id, d.client_id, d.kind_id, k.name, d.title, d.token, d.declined_at,
            d.decline_note, d.withdrawn_at, d.created_at, d.updated_at
       FROM documents d JOIN document_kinds k ON k.id = d.kind_id";

pub fn get_document(conn: &Connection, id: i64) -> Result<Document> {
    let mut document = conn
        .query_row(&format!("{DOCUMENT_SELECT} WHERE d.id = ?1"), [id], |r| {
            Ok(Document {
                id: r.get(0)?,
                client_id: r.get(1)?,
                kind_id: r.get(2)?,
                kind: r.get(3)?,
                title: r.get(4)?,
                token: r.get(5)?,
                declined_at: r.get(6)?,
                decline_note: r.get(7)?,
                withdrawn_at: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
                status: DocumentStatus::Draft,
            })
        })
        .optional()?
        .ok_or_else(|| NigelError::NotFound(format!("Document not found: id {id}")))?;
    document.status = status::document_status(conn, id)?;
    Ok(document)
}

fn version_from_row(r: &Row<'_>) -> rusqlite::Result<DocumentVersion> {
    Ok(DocumentVersion {
        id: r.get(0)?,
        document_id: r.get(1)?,
        number: r.get(2)?,
        file_path: r.get(3)?,
        checksum: r.get(4)?,
        sent_at: r.get(5)?,
        created_at: r.get(6)?,
    })
}

const VERSION_COLUMNS: &str = "id, document_id, number, file_path, checksum, sent_at, created_at";

pub fn versions(conn: &Connection, document_id: i64) -> Result<Vec<DocumentVersion>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {VERSION_COLUMNS} FROM document_versions WHERE document_id = ?1 ORDER BY number"
    ))?;
    let rows = stmt
        .query_map([document_id], version_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn latest_version(conn: &Connection, document_id: i64) -> Result<DocumentVersion> {
    conn.query_row(
        &format!(
            "SELECT {VERSION_COLUMNS} FROM document_versions
              WHERE document_id = ?1 ORDER BY number DESC LIMIT 1"
        ),
        [document_id],
        version_from_row,
    )
    .optional()?
    .ok_or_else(|| NigelError::NotFound(format!("Document not found: id {document_id}")))
}

pub fn latest_sent_version(conn: &Connection, document_id: i64) -> Result<Option<DocumentVersion>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {VERSION_COLUMNS} FROM document_versions
                  WHERE document_id = ?1 AND sent_at IS NOT NULL ORDER BY number DESC LIMIT 1"
            ),
            [document_id],
            version_from_row,
        )
        .optional()?)
}

fn corrupt(what: &str, value: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        format!("unknown {what} '{value}'").into(),
    )
}

pub fn recipients(conn: &Connection, version_id: i64) -> Result<Vec<Recipient>> {
    let mut stmt = conn.prepare(
        "SELECT id, version_id, role, name, email, token, position
           FROM document_recipients WHERE version_id = ?1 ORDER BY position, id",
    )?;
    let rows = stmt
        .query_map([version_id], |r| {
            let role: String = r.get(2)?;
            Ok(Recipient {
                id: r.get(0)?,
                version_id: r.get(1)?,
                role: RecipientRole::parse(&role)
                    .ok_or_else(|| corrupt("recipient role", &role))?,
                name: r.get(3)?,
                email: r.get(4)?,
                token: r.get(5)?,
                position: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn signatures(conn: &Connection, version_id: i64) -> Result<Vec<Signature>> {
    let mut stmt = conn.prepare(
        "SELECT id, version_id, recipient_id, role, name, email, method, signed_at,
                typed_name, ip, user_agent, checksum
           FROM document_signatures WHERE version_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([version_id], |r| {
            let role: String = r.get(3)?;
            let method: String = r.get(6)?;
            Ok(Signature {
                id: r.get(0)?,
                version_id: r.get(1)?,
                recipient_id: r.get(2)?,
                role: SignatureRole::parse(&role)
                    .ok_or_else(|| corrupt("signature role", &role))?,
                name: r.get(4)?,
                email: r.get(5)?,
                method: Method::parse(&method).ok_or_else(|| corrupt("method", &method))?,
                signed_at: r.get(7)?,
                typed_name: r.get(8)?,
                ip: r.get(9)?,
                user_agent: r.get(10)?,
                checksum: r.get(11)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn change_requests(conn: &Connection, version_id: i64) -> Result<Vec<ChangeRequest>> {
    let mut stmt = conn.prepare(
        "SELECT id, version_id, recipient_id, name, email, method, requested_at,
                note, ip, user_agent, checksum
           FROM document_change_requests WHERE version_id = ?1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([version_id], |r| {
            let method: String = r.get(5)?;
            Ok(ChangeRequest {
                id: r.get(0)?,
                version_id: r.get(1)?,
                recipient_id: r.get(2)?,
                name: r.get(3)?,
                email: r.get(4)?,
                method: Method::parse(&method).ok_or_else(|| corrupt("method", &method))?,
                requested_at: r.get(6)?,
                note: r.get(7)?,
                ip: r.get(8)?,
                user_agent: r.get(9)?,
                checksum: r.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn document_record(conn: &Connection, id: i64) -> Result<DocumentRecord> {
    let document = get_document(conn, id)?;
    let client_name: String = conn.query_row(
        "SELECT name FROM clients WHERE id = ?1",
        [document.client_id],
        |r| r.get(0),
    )?;
    let versions = versions(conn, id)?
        .into_iter()
        .map(|version| {
            Ok(VersionRecord {
                recipients: recipients(conn, version.id)?,
                signatures: signatures(conn, version.id)?,
                change_requests: change_requests(conn, version.id)?,
                version,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DocumentRecord {
        document,
        client_name,
        versions,
    })
}

#[derive(Debug, Default, Clone)]
pub struct DocumentFilter {
    pub client_id: Option<i64>,
    pub status: Option<DocumentStatus>,
    pub kind: Option<String>,
}

pub fn list_documents(conn: &Connection, filter: &DocumentFilter) -> Result<Vec<DocumentListRow>> {
    let mut stmt = conn.prepare(
        "SELECT d.id, d.title, k.name, d.client_id, c.name, lv.number, lv.sent_at, d.updated_at
           FROM documents d
           JOIN document_kinds k ON k.id = d.kind_id
           LEFT JOIN clients c ON c.id = d.client_id
           JOIN document_versions lv
             ON lv.document_id = d.id
            AND lv.number = (SELECT MAX(number) FROM document_versions WHERE document_id = d.id)
          WHERE (?1 IS NULL OR d.client_id = ?1)
            AND (?2 IS NULL OR k.name = ?2 COLLATE NOCASE)
          ORDER BY d.updated_at DESC, d.id DESC",
    )?;
    let raw = stmt
        .query_map(params![filter.client_id, filter.kind], |r| {
            Ok(DocumentListRow {
                id: r.get(0)?,
                title: r.get(1)?,
                kind: r.get(2)?,
                client_id: r.get(3)?,
                client_name: r.get(4)?,
                status: DocumentStatus::Draft,
                latest_version: r.get(5)?,
                sent_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut rows = Vec::with_capacity(raw.len());
    for mut row in raw {
        row.status = status::document_status(conn, row.id)?;
        if filter.status.is_none_or(|wanted| wanted == row.status) {
            rows.push(row);
        }
    }
    Ok(rows)
}

#[derive(Debug, Default, Clone)]
pub struct DocumentUpdate {
    pub title: Option<String>,
    pub kind: Option<String>,
}

pub fn update_document(
    conn: &Connection,
    id: i64,
    update: &DocumentUpdate,
    today: &str,
) -> Result<()> {
    let document = get_document(conn, id)?;
    guards::ensure_allowed(id, document.status, Action::Edit)?;
    let title = match &update.title {
        Some(t) => clean_title(t)?,
        None => document.title,
    };
    let kind_id = match &update.kind {
        Some(k) => active_kind_by_name(conn, k)?.id,
        None => document.kind_id,
    };
    let today = validate_date(today, "update date")?;
    conn.execute(
        "UPDATE documents SET title = ?1, kind_id = ?2, updated_at = ?3 WHERE id = ?4",
        params![title, kind_id, today, id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::checksum_of;
    use crate::documents::testing::{fixture_pdf, seed_client, seed_document, test_conn};

    #[test]
    fn ensure_pdf_refuses_content_that_only_claims_to_be_a_pdf() {
        assert!(ensure_pdf(&fixture_pdf("a")).is_ok());
        for bytes in [
            &b"<!doctype html><p>%PDF-1.7</p>"[..],
            &b"\x89PNG\r\n\x1a\n...."[..],
            &b"   %PDF-1.4\n"[..],
            &b"%PDF"[..],
            &b""[..],
        ] {
            let err = ensure_pdf(bytes).unwrap_err();
            assert!(matches!(err, NigelError::Invalid(_)), "{bytes:?}");
        }
    }

    #[test]
    fn filing_creates_a_draft_with_its_file_and_checksum() {
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let pdf = fixture_pdf("proposal");
        let id = file_document(
            &conn,
            dir.path(),
            &NewDocument {
                client_id: client,
                kind: "proposal",
                title: "Website rebuild",
            },
            &pdf,
            "2026-10-05",
        )
        .unwrap();
        let record = document_record(&conn, id).unwrap();
        assert_eq!(record.document.status, DocumentStatus::Draft);
        assert_eq!(record.document.kind, "Proposal");
        assert_eq!(record.versions.len(), 1);
        let v1 = &record.versions[0].version;
        assert_eq!(v1.checksum, checksum_of(&pdf));
        assert_eq!(std::fs::read(dir.path().join(&v1.file_path)).unwrap(), pdf);
        assert!(v1.sent_at.is_none());
    }

    #[test]
    fn the_stored_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let id = seed_document(&conn, dir.path(), client, "Private");
        let v = latest_version(&conn, id).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir.path().join(&v.file_path)), 0o600);
        assert_eq!(mode(&dir.path().join("documents")), 0o700);
        assert_eq!(mode(&dir.path().join(format!("documents/{id}"))), 0o700);
    }

    #[test]
    fn a_failed_file_write_leaves_neither_row() {
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        std::fs::write(dir.path().join("documents"), b"in the way").unwrap();
        let result = file_document(
            &conn,
            dir.path(),
            &NewDocument {
                client_id: client,
                kind: "Proposal",
                title: "Blocked",
            },
            &fixture_pdf("blocked"),
            "2026-10-05",
        );
        assert!(result.is_err());
        for table in ["documents", "document_versions"] {
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0, "{table}");
        }
    }

    #[test]
    fn the_same_pdf_for_the_same_client_is_a_conflict_naming_the_document() {
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let pdf = fixture_pdf("same");
        let first = file_document(
            &conn,
            dir.path(),
            &NewDocument {
                client_id: client,
                kind: "Proposal",
                title: "Website rebuild",
            },
            &pdf,
            "2026-10-05",
        )
        .unwrap();
        let err = file_document(
            &conn,
            dir.path(),
            &NewDocument {
                client_id: client,
                kind: "Proposal",
                title: "Again",
            },
            &pdf,
            "2026-10-05",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            NigelError::Conflict {
                code: "duplicate_document",
                ..
            }
        ));
        assert!(err.to_string().contains(&format!("#{first}")));
        let other = seed_client(&conn, "Juniper Labs");
        assert!(file_document(
            &conn,
            dir.path(),
            &NewDocument {
                client_id: other,
                kind: "Proposal",
                title: "Theirs"
            },
            &pdf,
            "2026-10-05"
        )
        .is_ok());
    }

    #[test]
    fn a_withdrawn_or_declined_document_does_not_hold_its_pdf() {
        use crate::documents::model::{gen_document_token, NewRecipient, RecipientRole};
        use crate::documents::record::{
            freeze_recipients, mark_sent, record_decline, record_withdrawal,
        };
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let pdf = fixture_pdf("same");
        let file = |title: &str| {
            file_document(
                &conn,
                dir.path(),
                &NewDocument {
                    client_id: client,
                    kind: "Proposal",
                    title,
                },
                &pdf,
                "2026-10-05",
            )
        };

        let withdrawn = file("Website rebuild").unwrap();
        record_withdrawal(&conn, withdrawn, "2026-10-05").unwrap();
        let declined = file("Website rebuild, to the right address").expect("refiled");

        let v = latest_version(&conn, declined).unwrap();
        freeze_recipients(
            &conn,
            v.id,
            &[(
                NewRecipient::new(RecipientRole::Signer, "Pat Example", "pat@cedar.test"),
                gen_document_token(),
            )],
        )
        .unwrap();
        mark_sent(&conn, v.id, "2026-10-05").unwrap();
        record_decline(&conn, declined, None, "2026-10-06").unwrap();
        let live = file("Website rebuild, once more").expect("refiled after a decline");

        let err = file("A fourth time").unwrap_err();
        assert!(
            err.to_string().contains(&format!("#{live}")),
            "the live document still holds it: {err}"
        );
    }

    #[test]
    fn an_archived_client_refuses_a_new_document() {
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Harbor & Vale");
        crate::invoicing::clients::archive_client(&conn, client, "2026-10-01").unwrap();
        let err = file_document(
            &conn,
            dir.path(),
            &NewDocument {
                client_id: client,
                kind: "Agreement",
                title: "MSA",
            },
            &fixture_pdf("x"),
            "2026-10-05",
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("unarchive it before filing or sending documents"));
        assert!(!dir.path().join("documents").exists());
    }

    #[test]
    fn the_list_filters_by_client_status_and_kind() {
        let (dir, conn) = test_conn();
        let cedar = seed_client(&conn, "Cedar Systems");
        let juniper = seed_client(&conn, "Juniper Labs");
        seed_document(&conn, dir.path(), cedar, "One");
        seed_document(&conn, dir.path(), juniper, "Two");
        let all = list_documents(&conn, &DocumentFilter::default()).unwrap();
        assert_eq!(all.len(), 2);
        let cedar_only = list_documents(
            &conn,
            &DocumentFilter {
                client_id: Some(cedar),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            cedar_only
                .iter()
                .map(|r| r.title.as_str())
                .collect::<Vec<_>>(),
            ["One"]
        );
        let sent = list_documents(
            &conn,
            &DocumentFilter {
                status: Some(DocumentStatus::Sent),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(sent.is_empty());
        let drafts = list_documents(
            &conn,
            &DocumentFilter {
                status: Some(DocumentStatus::Draft),
                kind: Some("proposal".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(drafts.len(), 2);
    }

    #[test]
    fn only_a_draft_takes_a_new_title_or_kind() {
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let id = seed_document(&conn, dir.path(), client, "Draft title");
        update_document(
            &conn,
            id,
            &DocumentUpdate {
                title: Some("Final title".into()),
                kind: Some("Agreement".into()),
            },
            "2026-10-06",
        )
        .unwrap();
        let doc = get_document(&conn, id).unwrap();
        assert_eq!(
            (
                doc.title.as_str(),
                doc.kind.as_str(),
                doc.updated_at.as_str()
            ),
            ("Final title", "Agreement", "2026-10-06")
        );
        conn.execute(
            "UPDATE document_versions SET sent_at = '2026-10-07' WHERE document_id = ?1",
            [id],
        )
        .unwrap();
        let err = update_document(
            &conn,
            id,
            &DocumentUpdate {
                title: Some("Late".into()),
                kind: None,
            },
            "2026-10-08",
        )
        .unwrap_err();
        assert!(matches!(
            err,
            NigelError::Conflict {
                code: "document_wrong_state",
                ..
            }
        ));
    }

    #[test]
    fn a_pdf_changed_on_disk_is_refused_rather_than_sent() {
        let (dir, conn) = test_conn();
        let client = seed_client(&conn, "Cedar Systems");
        let id = seed_document(&conn, dir.path(), client, "Tampered");
        let v = latest_version(&conn, id).unwrap();
        assert!(read_version_pdf(dir.path(), &v).is_ok());
        std::fs::write(dir.path().join(&v.file_path), fixture_pdf("other")).unwrap();
        assert!(matches!(
            read_version_pdf(dir.path(), &v),
            Err(NigelError::Conflict {
                code: "file_changed",
                ..
            })
        ));
    }
}
