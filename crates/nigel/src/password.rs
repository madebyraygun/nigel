use std::path::Path;

use crate::db::open_connection;
use crate::error::Result;

/// Encrypt an unencrypted database with a new password.
/// Uses ATTACH + sqlcipher_export since the backup API requires matching keys.
pub fn encrypt_database(db_path: &Path, new_password: &str) -> Result<()> {
    let tmp_path = db_path.with_extension("db.encrypting");
    let tmp_str = tmp_path.to_string_lossy();
    let conn = open_connection(db_path, None)?;
    conn.execute(
        "ATTACH DATABASE ?1 AS encrypted KEY ?2",
        rusqlite::params![&*tmp_str, new_password],
    )?;
    conn.execute_batch("SELECT sqlcipher_export('encrypted');")?;
    conn.execute_batch("DETACH DATABASE encrypted;")?;
    drop(conn);
    crate::settings::restrict_file_permissions(&tmp_path)?;
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    std::fs::rename(&tmp_path, db_path)?;
    crate::settings::restrict_file_permissions(db_path)?;
    Ok(())
}

/// Decrypt an encrypted database (remove password).
/// Uses ATTACH + sqlcipher_export since the backup API requires matching keys.
pub fn decrypt_database(db_path: &Path, current_password: &str) -> Result<()> {
    let tmp_path = db_path.with_extension("db.decrypting");
    let tmp_str = tmp_path.to_string_lossy();
    let conn = open_connection(db_path, Some(current_password))?;
    conn.execute_batch("SELECT count(*) FROM sqlite_master;")?;
    conn.execute(
        "ATTACH DATABASE ?1 AS plaintext KEY ''",
        rusqlite::params![&*tmp_str],
    )?;
    conn.execute_batch("SELECT sqlcipher_export('plaintext');")?;
    conn.execute_batch("DETACH DATABASE plaintext;")?;
    drop(conn);
    crate::settings::restrict_file_permissions(&tmp_path)?;
    let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    std::fs::rename(&tmp_path, db_path)?;
    crate::settings::restrict_file_permissions(db_path)?;
    Ok(())
}

/// Change the password on an already-encrypted database.
pub fn rekey_database(db_path: &Path, current_password: &str, new_password: &str) -> Result<()> {
    let conn = open_connection(db_path, Some(current_password))?;
    conn.execute_batch("SELECT count(*) FROM sqlite_master;")?;
    conn.pragma_update(None, "rekey", new_password)?;
    Ok(())
}

// Tests mutate the global DB_PASSWORD mutex and must run with --test-threads=1.
// See also: db::tests, cli::backup::tests.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db, is_encrypted, set_db_password};

    #[test]
    fn test_encrypt_then_decrypt_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create plain DB
        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        assert!(!is_encrypted(&db_path).unwrap());

        // Encrypt it
        encrypt_database(&db_path, "mypass").unwrap();
        assert!(is_encrypted(&db_path).unwrap());

        // Verify data survived
        let conn = open_connection(&db_path, Some("mypass")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
        drop(conn);

        // Decrypt it
        decrypt_database(&db_path, "mypass").unwrap();
        assert!(!is_encrypted(&db_path).unwrap());

        // Verify data survived
        set_db_password(None);
        let conn = get_connection(&db_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_rekey_changes_password() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        // Create encrypted DB
        set_db_password(Some("old".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        set_db_password(None);

        // Change password
        rekey_database(&db_path, "old", "new").unwrap();

        // Old password should fail (use raw connection to avoid PRAGMA errors)
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.pragma_update(None, "key", "old").unwrap();
        assert!(conn
            .execute_batch("SELECT count(*) FROM sqlite_master;")
            .is_err());

        // New password should work
        let conn = open_connection(&db_path, Some("new")).unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM categories", [], |r| r.get(0))
            .unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_encrypt_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        set_db_password(Some("correct".into()));
        let conn = get_connection(&db_path).unwrap();
        init_db(&conn).unwrap();
        drop(conn);
        set_db_password(None);

        // Try opening with wrong password (use raw connection)
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.pragma_update(None, "key", "wrong").unwrap();
        assert!(conn
            .execute_batch("SELECT count(*) FROM sqlite_master;")
            .is_err());
    }
}
