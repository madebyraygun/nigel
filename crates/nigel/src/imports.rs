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
}

/// Every import ever recorded, newest first, each with the number of
/// transactions still attached to it.
pub fn list_imports(conn: &Connection) -> Result<Vec<ImportListItem>> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.filename, COALESCE(a.name, '(unknown)'), i.import_date, COUNT(t.id)
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
