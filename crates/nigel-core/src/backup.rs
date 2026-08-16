use std::path::Path;

use rusqlite::backup::Backup;

use crate::error::Result;
use crate::settings::restrict_file_permissions;

/// Copy the database to `dest_path` using SQLite's online-backup API.
/// Preserves encryption state: encrypted sources produce encrypted backups.
/// The `password` must match the source database's encryption key (if any).
pub fn snapshot(conn: &rusqlite::Connection, dest_path: &Path) -> Result<()> {
    snapshot_with_password(conn, dest_path, crate::db::get_db_password().as_deref())
}

/// Like `snapshot`, but accepts an explicit password instead of reading global state.
/// Used by tests that cannot rely on the global DB_PASSWORD mutex.
pub fn snapshot_with_password(
    conn: &rusqlite::Connection,
    dest_path: &Path,
    password: Option<&str>,
) -> Result<()> {
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut dest_conn = rusqlite::Connection::open(dest_path)?;
    if let Some(pw) = password {
        dest_conn.pragma_update(None, "key", pw)?;
    }
    let backup = Backup::new(conn, &mut dest_conn)?;
    backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
    drop(backup);
    drop(dest_conn);
    restrict_file_permissions(dest_path)?;
    Ok(())
}
