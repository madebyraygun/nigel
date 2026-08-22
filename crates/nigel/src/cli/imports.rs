use rusqlite::Connection;

use nigel_core::db::get_connection;
use nigel_core::error::{NigelError, Result};
use nigel_core::imports::{
    import_exists, list_imports, list_rejects, ImportListItem, ImportReject,
};
use nigel_core::settings::get_data_dir;

/// Every import, newest first, with what it kept and what it dropped.
pub fn list() -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    let items = list_imports(&conn)?;

    if items.is_empty() {
        println!("No imports yet.");
        return Ok(());
    }

    for item in &items {
        println!("{}", history_line(item));
    }
    Ok(())
}

/// What an import kept, and what it dropped when it dropped anything.
pub fn counts_label(item: &ImportListItem) -> String {
    if item.malformed_count > 0 {
        format!(
            "{} rows, {} dropped",
            item.transaction_count, item.malformed_count
        )
    } else {
        format!("{} rows", item.transaction_count)
    }
}

/// One import as the history prints it.
pub fn history_line(item: &ImportListItem) -> String {
    format!(
        "{:<4} {:<25} {:<26} {:<22} {}",
        item.id,
        item.filename,
        item.account_name,
        item.import_date,
        counts_label(item)
    )
}

/// The rows one import could not parse.
pub fn rejects(id: i64) -> Result<()> {
    let conn = get_connection(&get_data_dir().join("nigel.db"))?;
    for line in rejects_report(&conn, id)? {
        println!("{line}");
    }
    Ok(())
}

/// What `nigel imports rejects` prints, or the error it refuses with.
///
/// An id no import has is a `NotFound`, never "dropped no rows": a clean import
/// and an import that does not exist are different facts, and only one of them
/// means the file made it in. `GET /api/imports/{id}/rejects` answers the same
/// way, from the same probe.
fn rejects_report(conn: &Connection, id: i64) -> Result<Vec<String>> {
    if !import_exists(conn, id)? {
        return Err(NigelError::NotFound(format!("No import with ID {id}")));
    }

    let rejects = list_rejects(conn, id)?;
    if rejects.is_empty() {
        return Ok(vec![format!("Import {id} dropped no rows.")]);
    }

    let mut lines = vec![format!("Import {id} dropped {} rows:", rejects.len())];
    lines.extend(reject_lines(&rejects));
    Ok(lines)
}

/// Two lines per reject: what failed, then the row it failed on.
pub fn reject_lines(rejects: &[ImportReject]) -> Vec<String> {
    let mut lines = Vec::with_capacity(rejects.len() * 2);
    for reject in rejects {
        lines.push(format!("  line {:<3} {}", reject.row_number, reject.reason));
        lines.push(format!("           {}", reject.content));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use nigel_core::db::init_db;

    /// A database with one import that dropped nothing.
    fn conn_with_import() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn.execute(
            "INSERT INTO imports (id, filename, account_id, import_date, record_count)
             VALUES (7, 'march.csv', NULL, '2026-03-02 09:14:11', 42)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn an_id_no_import_has_is_refused_rather_than_reported_as_clean() {
        let conn = conn_with_import();

        let err = rejects_report(&conn, 99).unwrap_err();

        assert!(
            matches!(&err, NigelError::NotFound(msg) if msg == "No import with ID 99"),
            "{err:?}"
        );
    }

    #[test]
    fn an_import_that_dropped_nothing_says_so() {
        let conn = conn_with_import();

        assert_eq!(
            rejects_report(&conn, 7).unwrap(),
            vec!["Import 7 dropped no rows.".to_string()]
        );
    }

    fn item(filename: &str, transactions: i64, malformed: i64) -> ImportListItem {
        ImportListItem {
            id: 7,
            filename: filename.into(),
            account_name: "Cedar Systems Checking".into(),
            import_date: "2026-03-02 09:14:11".into(),
            transaction_count: transactions,
            malformed_count: malformed,
        }
    }

    #[test]
    fn a_history_line_puts_the_dropped_count_beside_the_records() {
        assert_eq!(
            counts_label(&item("march.csv", 42, 2)),
            "42 rows, 2 dropped"
        );
    }

    #[test]
    fn an_import_that_dropped_nothing_says_nothing_about_it() {
        assert_eq!(counts_label(&item("march.csv", 42, 0)), "42 rows");
    }

    #[test]
    fn a_history_line_carries_the_id_the_file_the_account_and_the_counts() {
        let line = history_line(&item("march.csv", 42, 2));

        assert!(line.starts_with("7 "), "{line}");
        assert!(line.contains("march.csv"), "{line}");
        assert!(line.contains("Cedar Systems Checking"), "{line}");
        assert!(line.contains("2026-03-02 09:14:11"), "{line}");
        assert!(line.ends_with("42 rows, 2 dropped"), "{line}");
    }

    #[test]
    fn a_reject_prints_its_line_its_reason_and_the_row_itself() {
        let rejects = vec![ImportReject {
            id: 1,
            row_number: 7,
            content: "2026-03-09,GLOBEX HOSTING,-88.00".into(),
            reason: "date \"2026-03-09\" is not MM/DD/YYYY".into(),
        }];

        assert_eq!(
            reject_lines(&rejects),
            vec![
                "  line 7   date \"2026-03-09\" is not MM/DD/YYYY".to_string(),
                "           2026-03-09,GLOBEX HOSTING,-88.00".to_string(),
            ]
        );
    }
}
