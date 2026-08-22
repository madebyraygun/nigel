use rusqlite::Connection;
use serde::Serialize;

use crate::error::Result;

/// Information about the most recent import, used for display and deletion.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastImport {
    pub import_id: i64,
    pub filename: String,
    pub account_name: String,
    pub import_date: String,
    pub transaction_count: i64,
}

/// One row of import history.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportListItem {
    pub id: i64,
    pub filename: String,
    pub account_name: String,
    pub import_date: String,
    pub transaction_count: i64,
    /// How many rows the parser refused. The rows themselves are in
    /// `import_rejects`, one per count.
    pub malformed_count: i64,
}

/// Every import ever recorded, newest first, each with the number of
/// transactions still attached to it.
pub fn list_imports(conn: &Connection) -> Result<Vec<ImportListItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.filename, COALESCE(a.name, '(unknown)'), i.import_date, COUNT(t.id),
                i.malformed_count
         FROM imports i
         LEFT JOIN accounts a ON a.id = i.account_id
         LEFT JOIN transactions t ON t.import_id = i.id
         GROUP BY i.id
         ORDER BY i.id DESC",
    )?;

    let imports = stmt
        .query_map([], |row| {
            Ok(ImportListItem {
                id: row.get(0)?,
                filename: row.get(1)?,
                account_name: row.get(2)?,
                import_date: row.get(3)?,
                transaction_count: row.get(4)?,
                malformed_count: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(imports)
}

/// Query the most recent import and its associated transaction count.
/// Returns None if there are no imports in the database.
pub fn get_last_import(conn: &Connection) -> Result<Option<LastImport>> {
    Ok(list_imports(conn)?
        .into_iter()
        .next()
        .map(|item| LastImport {
            import_id: item.id,
            filename: item.filename,
            account_name: item.account_name,
            import_date: item.import_date,
            transaction_count: item.transaction_count,
        }))
}

/// Whether an import record still exists. `delete_import` reports a missing
/// import as zero transactions deleted, which over HTTP would read as success.
pub fn import_exists(conn: &Connection, import_id: i64) -> Result<bool> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM imports WHERE id = ?1)",
        [import_id],
        |row| row.get(0),
    )?;
    Ok(exists)
}

/// Delete all transactions and the import record for the given import.
/// Returns the number of transactions deleted.
pub fn delete_import(conn: &Connection, import_id: i64) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let deleted = tx.execute("DELETE FROM transactions WHERE import_id = ?1", [import_id])?;
    tx.execute("DELETE FROM imports WHERE id = ?1", [import_id])?;
    tx.commit()?;
    Ok(deleted)
}

/// One row an import could not parse.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReject {
    pub id: i64,
    /// The line in the file, as the parser counted it.
    pub row_number: i64,
    pub content: String,
    pub reason: String,
}

/// The rows one import dropped, in the order they appeared in the file.
pub fn list_rejects(conn: &Connection, import_id: i64) -> Result<Vec<ImportReject>> {
    let mut stmt = conn.prepare(
        "SELECT id, row_number, content, reason FROM import_rejects \
         WHERE import_id = ?1 ORDER BY row_number, id",
    )?;
    let rejects = stmt
        .query_map([import_id], |row| {
            Ok(ImportReject {
                id: row.get(0)?,
                row_number: row.get(1)?,
                content: row.get(2)?,
                reason: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rejects)
}

/// An account whose imports dropped rows, and how many.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedRows {
    pub account_name: String,
    pub malformed_count: i64,
}

/// Accounts with incomplete books, worst first. Empty when nothing was ever
/// dropped, which is what makes it worth printing only when it is not.
pub fn dropped_rows_by_account(conn: &Connection) -> Result<Vec<DroppedRows>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(a.name, '(unknown)'), SUM(i.malformed_count)
         FROM imports i
         LEFT JOIN accounts a ON a.id = i.account_id
         GROUP BY i.account_id
         HAVING SUM(i.malformed_count) > 0
         ORDER BY SUM(i.malformed_count) DESC, 1",
    )?;
    let dropped = stmt
        .query_map([], |row| {
            Ok(DroppedRows {
                account_name: row.get(0)?,
                malformed_count: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn seeded() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO accounts (name, account_type) VALUES ('Cedar Systems Checking', 'checking');
             INSERT INTO accounts (name, account_type) VALUES ('Globex Card', 'credit_card');
             INSERT INTO imports (filename, account_id, record_count, checksum, malformed_count)
                 VALUES ('march.csv', 1, 4, 'a1b2c3', 2);
             INSERT INTO imports (filename, account_id, record_count, checksum, malformed_count)
                 VALUES ('april.csv', 2, 6, 'd4e5f6', 1);
             INSERT INTO import_rejects (import_id, row_number, content, reason)
                 VALUES (1, 7, '2026-03-09,GLOBEX HOSTING,-88.00', 'date \"2026-03-09\" is not MM/DD/YYYY'),
                        (1, 9, '03/11/2026,INITECH LICENSE,n/a', 'amount \"n/a\" is not a number'),
                        (2, 4, '04/02/2026,HARBOR & VALE,,', 'amount \"\" is not a number');",
        )
        .unwrap();
        (dir, conn)
    }

    #[test]
    fn the_history_carries_the_count_of_what_each_import_dropped() {
        let (_dir, conn) = seeded();
        let items = list_imports(&conn).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].filename, "april.csv");
        assert_eq!(items[0].malformed_count, 1);
        assert_eq!(items[1].filename, "march.csv");
        assert_eq!(items[1].malformed_count, 2);
    }

    #[test]
    fn an_imports_rejects_read_back_in_file_order() {
        let (_dir, conn) = seeded();
        let rejects = list_rejects(&conn, 1).unwrap();

        assert_eq!(rejects.len(), 2);
        assert_eq!(rejects[0].row_number, 7);
        assert_eq!(rejects[0].content, "2026-03-09,GLOBEX HOSTING,-88.00");
        assert_eq!(rejects[0].reason, "date \"2026-03-09\" is not MM/DD/YYYY");
        assert_eq!(rejects[1].row_number, 9);
        assert!(list_rejects(&conn, 999).unwrap().is_empty());
    }

    #[test]
    fn dropped_rows_are_totalled_per_account_worst_first() {
        let (_dir, conn) = seeded();
        let dropped = dropped_rows_by_account(&conn).unwrap();

        assert_eq!(dropped.len(), 2);
        assert_eq!(dropped[0].account_name, "Cedar Systems Checking");
        assert_eq!(dropped[0].malformed_count, 2);
        assert_eq!(dropped[1].account_name, "Globex Card");
        assert_eq!(dropped[1].malformed_count, 1);
    }

    #[test]
    fn whole_books_report_nothing_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();

        assert!(dropped_rows_by_account(&conn).unwrap().is_empty());
    }
}
