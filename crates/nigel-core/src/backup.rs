use std::path::{Path, PathBuf};

use rusqlite::backup::Backup;

use crate::error::Result;
use crate::settings::{restrict_dir_permissions, restrict_file_permissions};

/// Copy the database to `dest_path` using SQLite's online-backup API.
/// Preserves encryption state: encrypted sources produce encrypted backups.
/// The `password` must match the source database's encryption key (if any).
pub fn snapshot(conn: &rusqlite::Connection, dest_path: &Path) -> Result<()> {
    snapshot_with_password(conn, dest_path, crate::db::get_db_password().as_deref())
}

/// The folder holding the filed document PDFs that go with the backup at
/// `backup_path`: the same path with its extension replaced by `documents`.
pub fn documents_path_for(backup_path: &Path) -> PathBuf {
    backup_path.with_extension("documents")
}

/// Copy `<data_dir>/documents/` to the folder that accompanies `backup_path`.
/// Returns the number of files copied; a data directory with no `documents/`
/// folder copies nothing and creates nothing.
pub fn copy_documents_out(data_dir: &Path, backup_path: &Path) -> Result<usize> {
    let src = data_dir.join("documents");
    if !src.is_dir() {
        return Ok(0);
    }
    copy_tree(&src, &documents_path_for(backup_path))
}

/// Replace `<data_dir>/documents/` with the folder that accompanies
/// `backup_path`. Returns `None` and touches nothing when the backup has no
/// such folder.
pub fn restore_documents_in(data_dir: &Path, backup_path: &Path) -> Result<Option<usize>> {
    let src = documents_path_for(backup_path);
    if !src.is_dir() {
        return Ok(None);
    }
    let dest = data_dir.join("documents");
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    copy_tree(&src, &dest).map(Some)
}

fn copy_tree(src: &Path, dest: &Path) -> Result<usize> {
    std::fs::create_dir_all(dest)?;
    restrict_dir_permissions(dest)?;
    let mut copied = 0;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copied += copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
            restrict_file_permissions(&target)?;
            copied += 1;
        }
    }
    Ok(copied)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_round_trip_through_a_backup() {
        let data = tempfile::tempdir().unwrap();
        let docs = data.path().join("documents/1");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("v1.pdf"), b"%PDF-1.4 one").unwrap();
        std::fs::write(docs.join("v2.pdf"), b"%PDF-1.4 two").unwrap();

        let out = tempfile::tempdir().unwrap();
        let backup = out.path().join("nigel-test.db");
        assert_eq!(copy_documents_out(data.path(), &backup).unwrap(), 2);
        assert!(documents_path_for(&backup).join("1/v2.pdf").is_file());

        let fresh = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(fresh.path().join("documents/9")).unwrap();
        std::fs::write(fresh.path().join("documents/9/v1.pdf"), b"stale").unwrap();
        assert_eq!(
            restore_documents_in(fresh.path(), &backup).unwrap(),
            Some(2)
        );
        assert_eq!(
            std::fs::read(fresh.path().join("documents/1/v1.pdf")).unwrap(),
            b"%PDF-1.4 one"
        );
        assert!(!fresh.path().join("documents/9").exists());
    }

    #[test]
    fn a_backup_without_documents_leaves_them_alone() {
        let data = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let backup = out.path().join("nigel-test.db");
        assert_eq!(copy_documents_out(data.path(), &backup).unwrap(), 0);
        assert!(!documents_path_for(&backup).exists());

        std::fs::create_dir_all(data.path().join("documents/1")).unwrap();
        std::fs::write(data.path().join("documents/1/v1.pdf"), b"keep").unwrap();
        assert_eq!(restore_documents_in(data.path(), &backup).unwrap(), None);
        assert!(data.path().join("documents/1/v1.pdf").is_file());
    }
}
