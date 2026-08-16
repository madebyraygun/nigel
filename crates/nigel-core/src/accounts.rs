use rusqlite::Connection;

use crate::error::{DeleteBlock, NigelError, Result};
use crate::models::Account;

/// The account types the TUI offers and the data layer accepts.
pub const ACCOUNT_TYPES: &[&str] = &["checking", "credit_card", "line_of_credit", "payroll"];

// ---------------------------------------------------------------------------
// Data-layer functions for TUI account management
// ---------------------------------------------------------------------------

pub fn list_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, account_type, institution, last_four FROM accounts ORDER BY name",
    )?;
    let accounts = stmt
        .query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: row.get(2)?,
                institution: row.get(3)?,
                last_four: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(accounts)
}

pub fn get_account(conn: &Connection, id: i64) -> Result<Account> {
    conn.query_row(
        "SELECT id, name, account_type, institution, last_four FROM accounts WHERE id = ?1",
        [id],
        |row| {
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: row.get(2)?,
                institution: row.get(3)?,
                last_four: row.get(4)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            NigelError::NotFound(format!("Account not found: id {id}"))
        }
        other => NigelError::Db(other),
    })
}

/// Insert an account and return its id.
pub fn add_account(
    conn: &Connection,
    name: &str,
    account_type: &str,
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
        "INSERT INTO accounts (name, account_type, institution, last_four) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, account_type, institution, last_four],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_account(conn: &Connection, id: i64, new_name: &str) -> Result<()> {
    if new_name.trim().is_empty() {
        return Err(NigelError::Invalid("Name is required".into()));
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE name = ?1 AND id != ?2)",
        rusqlite::params![new_name, id],
        |row| row.get(0),
    )?;
    if exists {
        return Err(NigelError::DuplicateName {
            kind: "Account",
            name: new_name.to_string(),
        });
    }
    let updated = conn.execute(
        "UPDATE accounts SET name = ?1 WHERE id = ?2",
        rusqlite::params![new_name, id],
    )?;
    if updated == 0 {
        return Err(NigelError::NotFound(format!("Account not found: id {id}")));
    }
    Ok(())
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

pub fn delete_account(conn: &Connection, id: i64) -> Result<()> {
    if let Some(block) = delete_blocker(conn, id)? {
        return Err(NigelError::Blocked(block));
    }
    // Clean up reconciliations; null out imports to preserve checksums for duplicate detection
    conn.execute("DELETE FROM reconciliations WHERE account_id = ?1", [id])?;
    conn.execute(
        "UPDATE imports SET account_id = NULL WHERE account_id = ?1",
        [id],
    )?;
    let deleted = conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
    if deleted == 0 {
        return Err(NigelError::NotFound(format!("Account not found: id {id}")));
    }
    Ok(())
}
