//! Document status is derived from the rows on every read and never stored.
use rusqlite::{Connection, OptionalExtension};

use crate::documents::model::DocumentStatus;
use crate::error::{NigelError, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusFacts {
    pub withdrawn: bool,
    pub declined: bool,
    pub countersigned: bool,
    pub latest_is_sent: bool,
    pub client_signed_latest: bool,
    pub change_requested_latest: bool,
}

/// A newer unsent version reads as `Draft`: after a revise the change request
/// still sits on the previous version, and the document must be sendable again.
pub fn status_from(f: &StatusFacts) -> DocumentStatus {
    use DocumentStatus::*;
    if f.withdrawn {
        return Withdrawn;
    }
    if f.countersigned {
        return Executed;
    }
    if f.declined {
        return Declined;
    }
    if !f.latest_is_sent {
        return Draft;
    }
    if f.client_signed_latest {
        return Accepted;
    }
    if f.change_requested_latest {
        return ChangesRequested;
    }
    Sent
}

const FACTS_SQL: &str = "
    SELECT d.withdrawn_at IS NOT NULL,
           d.declined_at IS NOT NULL,
           EXISTS(SELECT 1 FROM document_signatures s
                    JOIN document_versions v ON v.id = s.version_id
                   WHERE v.document_id = d.id AND s.role = 'countersign'),
           lv.sent_at IS NOT NULL,
           EXISTS(SELECT 1 FROM document_signatures s
                   WHERE s.version_id = lv.id AND s.role = 'client'),
           EXISTS(SELECT 1 FROM document_change_requests c WHERE c.version_id = lv.id)
      FROM documents d
      JOIN document_versions lv
        ON lv.document_id = d.id
       AND lv.number = (SELECT MAX(number) FROM document_versions WHERE document_id = d.id)
     WHERE d.id = ?1";

pub fn status_facts(conn: &Connection, document_id: i64) -> Result<StatusFacts> {
    conn.query_row(FACTS_SQL, [document_id], |r| {
        Ok(StatusFacts {
            withdrawn: r.get(0)?,
            declined: r.get(1)?,
            countersigned: r.get(2)?,
            latest_is_sent: r.get(3)?,
            client_signed_latest: r.get(4)?,
            change_requested_latest: r.get(5)?,
        })
    })
    .optional()?
    .ok_or_else(|| NigelError::NotFound(format!("Document not found: id {document_id}")))
}

pub fn document_status(conn: &Connection, document_id: i64) -> Result<DocumentStatus> {
    Ok(status_from(&status_facts(conn, document_id)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::DocumentStatus;

    fn facts() -> StatusFacts {
        StatusFacts::default()
    }

    #[test]
    fn every_row_of_the_status_table() {
        use DocumentStatus::*;
        let cases = [
            (facts(), Draft),
            (
                StatusFacts {
                    latest_is_sent: true,
                    ..facts()
                },
                Sent,
            ),
            (
                StatusFacts {
                    latest_is_sent: true,
                    change_requested_latest: true,
                    ..facts()
                },
                ChangesRequested,
            ),
            (
                StatusFacts {
                    latest_is_sent: true,
                    client_signed_latest: true,
                    ..facts()
                },
                Accepted,
            ),
            (
                StatusFacts {
                    latest_is_sent: true,
                    client_signed_latest: true,
                    change_requested_latest: true,
                    ..facts()
                },
                Accepted,
            ),
            (
                StatusFacts {
                    latest_is_sent: true,
                    declined: true,
                    ..facts()
                },
                Declined,
            ),
            (
                StatusFacts {
                    latest_is_sent: true,
                    client_signed_latest: true,
                    countersigned: true,
                    ..facts()
                },
                Executed,
            ),
            (
                StatusFacts {
                    countersigned: true,
                    declined: true,
                    ..facts()
                },
                Executed,
            ),
            (
                StatusFacts {
                    withdrawn: true,
                    countersigned: true,
                    ..facts()
                },
                Withdrawn,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(status_from(&input), expected, "{input:?}");
        }
    }

    #[test]
    fn a_newer_unsent_version_reads_as_draft_whatever_the_older_one_carries() {
        let input = StatusFacts {
            latest_is_sent: false,
            change_requested_latest: true,
            client_signed_latest: true,
            ..facts()
        };
        assert_eq!(status_from(&input), DocumentStatus::Draft);
    }

    #[test]
    fn nothing_in_the_documents_module_writes_a_status() {
        let sources = [
            include_str!("model.rs"),
            include_str!("kinds.rs"),
            include_str!("status.rs"),
            include_str!("store.rs"),
            include_str!("record.rs"),
        ];
        for source in sources {
            assert!(
                !source.contains(concat!("SET ", "status")),
                "a status write"
            );
        }
    }

    #[test]
    fn the_documents_table_has_no_status_column() {
        let (_d, conn) = crate::documents::testing::test_conn();
        let has: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('documents') WHERE name = 'status'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!has);
    }

    #[test]
    fn the_facts_come_from_the_rows() {
        let (_d, conn) = crate::documents::testing::test_conn();
        conn.execute_batch(
            "INSERT INTO clients (name) VALUES ('Cedar Systems');
             INSERT INTO documents (client_id, kind_id, title, token, created_at, updated_at)
                 VALUES (1, 1, 'Proposal', 'tok', '2026-10-01', '2026-10-01');
             INSERT INTO document_versions (document_id, number, file_path, checksum, sent_at, created_at)
                 VALUES (1, 1, 'documents/1/v1.pdf', 'sha256:01', '2026-10-02', '2026-10-01');
             INSERT INTO document_change_requests (version_id, name, method, requested_at, note, checksum)
                 VALUES (1, 'Sam Example', 'manual', '2026-10-03', 'Fix the dates', 'sha256:01');",
        )
        .unwrap();
        assert_eq!(
            document_status(&conn, 1).unwrap(),
            DocumentStatus::ChangesRequested
        );
        conn.execute_batch(
            "INSERT INTO document_versions (document_id, number, file_path, checksum, created_at)
                 VALUES (1, 2, 'documents/1/v2.pdf', 'sha256:02', '2026-10-04');",
        )
        .unwrap();
        assert_eq!(document_status(&conn, 1).unwrap(), DocumentStatus::Draft);
    }
}
