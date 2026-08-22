use nigel_core::db::{get_connection, get_metadata, is_encrypted};
use nigel_core::error::Result;
use nigel_core::fmt::format_bytes;
use nigel_core::migrations::{get_schema_version, LATEST_VERSION};
use nigel_core::settings::load_settings;

pub fn run() -> Result<()> {
    let settings = load_settings();
    let data_dir = std::path::PathBuf::from(&settings.data_dir);
    let db_path = data_dir.join("nigel.db");

    let user_name = if settings.user_name.is_empty() {
        "(not set)"
    } else {
        &settings.user_name
    };

    if !db_path.exists() {
        println!("User:       {user_name}");
        println!("Data dir:   {}", data_dir.display());
        println!("Database:   {}", db_path.display());
        println!();
        println!("Database not found. Run `nigel init` to set up.");
        return Ok(());
    }

    // Collect all data before printing to avoid partial output on error
    let size = std::fs::metadata(&db_path)?.len();
    let encrypted = is_encrypted(&db_path)?;
    let conn = get_connection(&db_path)?;

    let company = get_metadata(&conn, "company_name");
    let schema_v = get_schema_version(&conn)?;
    let accounts: i64 = conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))?;
    let transactions: i64 =
        conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0))?;
    let flagged: i64 = conn.query_row(
        "SELECT count(*) FROM transactions WHERE is_flagged = 1",
        [],
        |r| r.get(0),
    )?;
    let rules: i64 = conn.query_row("SELECT count(*) FROM rules", [], |r| r.get(0))?;
    let dropped = nigel_core::imports::dropped_rows_by_account(&conn)?;

    // All data collected — print output
    println!("User:       {user_name}");
    println!("Data dir:   {}", data_dir.display());
    println!("Database:   {}", db_path.display());
    println!("DB size:    {}", format_bytes(size));
    println!("Encrypted:  {}", if encrypted { "yes" } else { "no" });
    println!("Company:    {}", company.as_deref().unwrap_or("(not set)"));
    println!("Schema:     v{schema_v} (latest: v{LATEST_VERSION})");
    println!();
    println!("Accounts:      {accounts}");
    println!("Transactions:  {transactions}");
    println!("Flagged:       {flagged}");
    println!("Rules:         {rules}");
    println!("{}", dropped_rows_line(&dropped));

    Ok(())
}

/// What the books are missing, if anything.
///
/// Named per account, because "some rows were dropped" is not actionable and
/// "Cedar Systems Checking dropped 2" points at the statement to re-import.
pub fn dropped_rows_line(dropped: &[nigel_core::imports::DroppedRows]) -> String {
    let total: i64 = dropped.iter().map(|d| d.malformed_count).sum();
    if total == 0 {
        return "Dropped rows:  0".to_string();
    }
    let detail = dropped
        .iter()
        .map(|d| format!("{} {}", d.account_name, d.malformed_count))
        .collect::<Vec<_>>()
        .join(", ");
    format!("Dropped rows:  {total} ({detail}) — run `nigel imports rejects <id>` to see them")
}

#[cfg(test)]
mod tests {
    use nigel_core::db::{get_connection, init_db, is_encrypted, open_connection, set_db_password};
    use nigel_core::imports::DroppedRows;

    #[test]
    fn whole_books_report_no_dropped_rows() {
        assert_eq!(super::dropped_rows_line(&[]), "Dropped rows:  0");
    }

    #[test]
    fn dropped_rows_name_the_accounts_they_came_from() {
        let dropped = vec![
            DroppedRows {
                account_name: "Cedar Systems Checking".into(),
                malformed_count: 2,
            },
            DroppedRows {
                account_name: "Globex Card".into(),
                malformed_count: 1,
            },
        ];

        assert_eq!(
            super::dropped_rows_line(&dropped),
            "Dropped rows:  3 (Cedar Systems Checking 2, Globex Card 1) \
             — run `nigel imports rejects <id>` to see them"
        );
    }

    #[test]
    fn test_is_encrypted_shown_false_for_plain_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = open_connection(&db_path, None).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        assert!(!is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_is_encrypted_shown_true_for_encrypted_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        assert!(is_encrypted(&db_path).unwrap());
    }

    #[test]
    fn test_encrypted_db_accessible_with_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create encrypted DB
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Set global password and verify get_connection works
        set_db_password(Some("secret".to_string()));
        let conn = get_connection(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);

        // Clean up global state
        set_db_password(None);
    }

    #[test]
    fn test_encrypted_db_fails_without_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create encrypted DB
        let conn = open_connection(&db_path, Some("secret")).unwrap();
        init_db(&conn).unwrap();
        drop(conn);

        // Ensure no global password is set
        set_db_password(None);
        // get_connection fails with "file is not a database" when password is missing
        let result = get_connection(&db_path);
        assert!(result.is_err());
    }
}
