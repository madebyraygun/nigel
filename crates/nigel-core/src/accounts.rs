use rusqlite::Connection;

use crate::db::AccountClass;
use crate::error::{DeleteBlock, NigelError, Result};
use crate::models::Account;

/// The account types the TUI offers and the data layer accepts.
pub const ACCOUNT_TYPES: &[&str] = &["checking", "credit_card", "line_of_credit", "payroll"];

// ---------------------------------------------------------------------------
// Data-layer functions for TUI account management
// ---------------------------------------------------------------------------

pub fn list_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, class, institution, last_four FROM accounts ORDER BY name",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, name, account_type, class, institution, last_four)| {
            Ok(Account {
                id,
                name,
                account_type,
                class: AccountClass::from_db(&class)?,
                institution,
                last_four,
            })
        })
        .collect()
}

pub fn get_account(conn: &Connection, id: i64) -> Result<Account> {
    let (id, name, account_type, class, institution, last_four) = conn
        .query_row(
            "SELECT id, name, account_type, class, institution, last_four FROM accounts WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                NigelError::NotFound(format!("Account not found: id {id}"))
            }
            other => NigelError::Db(other),
        })?;
    Ok(Account {
        id,
        name,
        account_type,
        class: AccountClass::from_db(&class)?,
        institution,
        last_four,
    })
}

/// Insert an account and return its id.
///
/// `class` absent means the account type decides. An explicit one is taken as
/// given: the derivation is a default, not a rule.
pub fn add_account(
    conn: &Connection,
    name: &str,
    account_type: &str,
    class: Option<AccountClass>,
    institution: Option<&str>,
    last_four: Option<&str>,
) -> Result<i64> {
    if name.trim().is_empty() {
        return Err(NigelError::Invalid("Name is required".into()));
    }
    if !ACCOUNT_TYPES.contains(&account_type) {
        return Err(NigelError::Invalid(format!(
            "Invalid account type: {account_type} (must be one of: {})",
            ACCOUNT_TYPES.join(", ")
        )));
    }
    let class = class.unwrap_or_else(|| crate::db::class_for_account_type(account_type));
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Account",
            name: name.to_string(),
        });
    }
    conn.execute(
        "INSERT INTO accounts (name, account_type, class, institution, last_four) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![name, account_type, class.as_str(), institution, last_four],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Change an account's name, its class, or both. `None` leaves a field alone.
pub fn update_account(
    conn: &Connection,
    id: i64,
    name: Option<&str>,
    class: Option<AccountClass>,
) -> Result<()> {
    if let Some(name) = name {
        if name.trim().is_empty() {
            return Err(NigelError::Invalid("Name is required".into()));
        }
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1 AND id != ?2)",
            rusqlite::params![name, id],
            |row| row.get(0),
        )?;
        if exists {
            return Err(NigelError::DuplicateName {
                kind: "Account",
                name: name.to_string(),
            });
        }
    }
    let updated = conn.execute(
        "UPDATE accounts SET name = COALESCE(?1, name), class = COALESCE(?2, class) WHERE id = ?3",
        rusqlite::params![name, class.map(|c| c.as_str()), id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!("Account not found: id {id}")));
    }
    Ok(())
}

pub fn rename_account(conn: &Connection, id: i64, new_name: &str) -> Result<()> {
    update_account(conn, id, Some(new_name), None)
}

pub fn transaction_count(conn: &Connection, account_id: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
        [account_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn account_names(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM accounts ORDER BY name")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut names = Vec::new();
    for row in rows {
        names.push(row?);
    }
    Ok(names)
}

/// Why this account cannot be deleted, or None when it can be. Separated from
/// `delete_account` so a caller can ask before it commits to trying.
pub fn delete_blocker(conn: &Connection, id: i64) -> Result<Option<DeleteBlock>> {
    let count = transaction_count(conn, id)?;
    if count > 0 {
        return Ok(Some(DeleteBlock::transactions("account", count)));
    }
    Ok(None)
}

/// Delete an account with no transactions, its reconciliations with it.
///
/// One `BEGIN IMMEDIATE` transaction holds the blocker check and all three
/// writes: an import landing in this account partway through either waits for
/// the write lock or fails the final delete, and a failure anywhere rolls the
/// reconciliations and imports back with it.
pub fn delete_account(conn: &Connection, id: i64) -> Result<()> {
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    if let Some(block) = delete_blocker(&tx, id)? {
        return Err(NigelError::Blocked(block));
    }
    // Clean up reconciliations; null out imports to preserve checksums for duplicate detection
    tx.execute("DELETE FROM reconciliations WHERE account_id = ?1", [id])?;
    tx.execute(
        "UPDATE imports SET account_id = NULL WHERE account_id = ?1",
        [id],
    )?;
    let deleted = tx.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
    if deleted == 0 {
        return Err(NigelError::NotFound(format!("Account not found: id {id}")));
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> (tempfile::TempDir, Connection) {
        crate::db::set_db_password(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = crate::db::open_connection(&dir.path().join("nigel.db"), None).expect("open");
        crate::db::init_db(&conn).expect("init");
        (dir, conn)
    }

    /// The account's reconciliations, and the imports still linked to it.
    fn history(conn: &Connection, id: i64) -> (i64, i64) {
        let count = |sql: &str| -> i64 { conn.query_row(sql, [id], |r| r.get(0)).unwrap() };
        (
            count("SELECT COUNT(*) FROM reconciliations WHERE account_id = ?1"),
            count("SELECT COUNT(*) FROM imports WHERE account_id = ?1"),
        )
    }

    fn account_with_history(conn: &Connection) -> i64 {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Acme Checking', 'checking')",
            [],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO reconciliations (account_id, month, statement_balance)
             VALUES (?1, '2026-01', 100.0)",
            [id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO imports (filename, account_id, checksum) VALUES ('acme.csv', ?1, 'abc')",
            [id],
        )
        .unwrap();
        id
    }

    /// The final delete fails after the blocker passed — what a transaction
    /// imported into the account between the two does through the foreign key.
    /// The reconciliations and the imports' account links come back with it.
    #[test]
    fn a_delete_that_fails_at_the_final_statement_leaves_the_history_alone() {
        let (_dir, conn) = test_db();
        let id = account_with_history(&conn);
        conn.execute_batch(
            "CREATE TEMP TRIGGER refuse_account_delete BEFORE DELETE ON accounts
             BEGIN SELECT RAISE(ABORT, 'FOREIGN KEY constraint failed'); END;",
        )
        .unwrap();

        let err = delete_account(&conn, id).expect_err("the final delete fails");
        assert!(matches!(err, NigelError::Db(_)), "{err:?}");

        assert_eq!(history(&conn, id), (1, 1), "history was not rolled back");
        assert!(conn.is_autocommit(), "the transaction was left open");
    }

    #[test]
    fn a_blocked_delete_leaves_the_history_alone() {
        let (_dir, conn) = test_db();
        let id = account_with_history(&conn);
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount)
             VALUES (?1, '2026-01-02', 'Globex', -10.0)",
            [id],
        )
        .unwrap();

        let err = delete_account(&conn, id).expect_err("blocked");
        assert!(matches!(err, NigelError::Blocked(_)), "{err:?}");
        assert_eq!(history(&conn, id), (1, 1));
        assert!(conn.is_autocommit());
    }

    #[test]
    fn a_clean_delete_commits_all_three_statements() {
        let (_dir, conn) = test_db();
        let id = account_with_history(&conn);

        delete_account(&conn, id).expect("deleted");

        assert_eq!(history(&conn, id), (0, 0));
        let unlinked: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM imports WHERE checksum = 'abc' AND account_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(unlinked, 1, "the checksum is kept for duplicate detection");
    }
}
