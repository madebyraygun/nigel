use std::path::{Path, PathBuf};

use rusqlite::backup::Backup;

use crate::error::{NigelError, Result};
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

/// Refuse a backup destination whose documents folder would sit inside (or
/// contain) the live `<data_dir>/documents/`.
pub fn check_documents_destination(data_dir: &Path, backup_path: &Path) -> Result<()> {
    let src = data_dir.join("documents");
    if !src.is_dir() {
        return Ok(());
    }
    let src = src.canonicalize()?;
    let dest = resolve_path(&documents_path_for(backup_path));
    if dest.starts_with(&src) || src.starts_with(&dest) {
        return Err(NigelError::Invalid(format!(
            "The backup's documents folder {} would overlap the live documents folder {}; choose a backup path outside it.",
            dest.display(),
            src.display()
        )));
    }
    Ok(())
}

/// Canonicalize the deepest existing ancestor of `path` and append the rest.
fn resolve_path(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut cur = path;
    loop {
        if let Ok(real) = cur.canonicalize() {
            return rest.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (cur.parent(), cur.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                cur = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn remove_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path)?,
        Ok(_) => std::fs::remove_file(path)?,
        Err(_) => {}
    }
    Ok(())
}

/// Copy `<data_dir>/documents/` to the folder that accompanies `backup_path`,
/// replacing any folder already there. Returns the number of files copied; a
/// data directory with no `documents/` folder copies nothing and creates
/// nothing.
pub fn copy_documents_out(data_dir: &Path, backup_path: &Path) -> Result<usize> {
    let src = data_dir.join("documents");
    if !src.is_dir() {
        return Ok(0);
    }
    check_documents_destination(data_dir, backup_path)?;
    let dest = documents_path_for(backup_path);
    let temp = sibling(&dest, ".tmp");
    remove_path(&temp)?;
    let copied = match copy_tree(&src, &temp) {
        Ok(n) => n,
        Err(e) => {
            let _ = remove_path(&temp);
            return Err(e);
        }
    };
    remove_path(&dest)?;
    std::fs::rename(&temp, &dest)?;
    Ok(copied)
}

/// Replace `<data_dir>/documents/` with the folder that accompanies
/// `backup_path`. Returns `None` and touches nothing when the backup has no
/// such folder. The copy is staged beside the live folder and swapped in by
/// rename, so a failure leaves the live folder as it was; the error names
/// `safety_backup` when there is one. A `documents` symlink is followed, so
/// the link survives and its target is what gets replaced.
pub fn restore_documents_in(
    data_dir: &Path,
    backup_path: &Path,
    safety_backup: Option<&Path>,
) -> Result<Option<usize>> {
    let src = documents_path_for(backup_path);
    if !src.is_dir() {
        return Ok(None);
    }
    let live = data_dir.join("documents");
    let target = if std::fs::symlink_metadata(&live).is_ok_and(|m| m.file_type().is_symlink()) {
        live.canonicalize()?
    } else {
        live
    };
    let staging = sibling(&target, ".restoring");
    let old = sibling(&target, ".replaced");
    let fail = |e: NigelError| {
        let hint = match safety_backup {
            Some(path) => format!(
                " The database was already replaced; to go back, restore the safety backup {}.",
                path.display()
            ),
            None => String::new(),
        };
        NigelError::Other(format!(
            "Could not restore the filed documents: {e}. The current documents folder was left as it was.{hint}"
        ))
    };

    remove_path(&staging).map_err(fail)?;
    remove_path(&old).map_err(fail)?;
    let copied = match copy_tree(&src, &staging) {
        Ok(n) => n,
        Err(e) => {
            let _ = remove_path(&staging);
            return Err(fail(e));
        }
    };
    let had_live = target.exists();
    if had_live {
        if let Err(e) = std::fs::rename(&target, &old) {
            let _ = remove_path(&staging);
            return Err(fail(e.into()));
        }
    }
    if let Err(e) = std::fs::rename(&staging, &target) {
        if had_live {
            let _ = std::fs::rename(&old, &target);
        }
        let _ = remove_path(&staging);
        return Err(fail(e.into()));
    }
    remove_path(&old)?;
    Ok(Some(copied))
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
            restore_documents_in(fresh.path(), &backup, None).unwrap(),
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
        assert_eq!(
            restore_documents_in(data.path(), &backup, None).unwrap(),
            None
        );
        assert!(data.path().join("documents/1/v1.pdf").is_file());
    }

    fn write_doc(data: &Path, rel: &str, body: &[u8]) {
        let path = data.join("documents").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn a_reused_backup_path_carries_no_stale_documents() {
        let data = tempfile::tempdir().unwrap();
        write_doc(data.path(), "1/v1.pdf", b"one");
        let out = tempfile::tempdir().unwrap();
        let backup = out.path().join("nigel.db");
        copy_documents_out(data.path(), &backup).unwrap();

        std::fs::remove_dir_all(data.path().join("documents/1")).unwrap();
        write_doc(data.path(), "2/v1.pdf", b"two");
        assert_eq!(copy_documents_out(data.path(), &backup).unwrap(), 1);
        let folder = documents_path_for(&backup);
        assert!(!folder.join("1").exists());
        assert!(folder.join("2/v1.pdf").is_file());
        assert!(!sibling(&folder, ".tmp").exists());
    }

    #[test]
    fn a_backup_destination_inside_the_live_documents_is_refused() {
        let data = tempfile::tempdir().unwrap();
        write_doc(data.path(), "1/v1.pdf", b"one");
        let inside = data.path().join("documents/1/nigel.db");
        let err = copy_documents_out(data.path(), &inside).unwrap_err();
        assert!(err.to_string().contains("overlap"), "{err}");
        assert!(check_documents_destination(data.path(), &inside).is_err());

        let outside = data.path().join("backups/nigel.db");
        assert!(check_documents_destination(data.path(), &outside).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_restore_copy_leaves_the_live_documents_untouched() {
        let data = tempfile::tempdir().unwrap();
        write_doc(data.path(), "1/v1.pdf", b"live");
        let out = tempfile::tempdir().unwrap();
        let backup = out.path().join("nigel.db");
        let folder = documents_path_for(&backup);
        std::fs::create_dir_all(&folder).unwrap();
        std::os::unix::fs::symlink(out.path().join("missing"), folder.join("broken.pdf")).unwrap();

        let safety = out.path().join("nigel-pre-restore.db");
        let err = restore_documents_in(data.path(), &backup, Some(&safety)).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("nigel-pre-restore.db"), "{message}");
        assert_eq!(
            std::fs::read(data.path().join("documents/1/v1.pdf")).unwrap(),
            b"live"
        );
        assert!(!data.path().join("documents.restoring").exists());
    }

    #[cfg(unix)]
    #[test]
    fn restore_follows_a_documents_symlink() {
        let data = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let real = elsewhere.path().join("pdfs");
        std::fs::create_dir_all(real.join("9")).unwrap();
        std::fs::write(real.join("9/v1.pdf"), b"stale").unwrap();
        std::os::unix::fs::symlink(&real, data.path().join("documents")).unwrap();

        let src = tempfile::tempdir().unwrap();
        write_doc(src.path(), "1/v1.pdf", b"fresh");
        let out = tempfile::tempdir().unwrap();
        let backup = out.path().join("nigel.db");
        copy_documents_out(src.path(), &backup).unwrap();

        assert_eq!(
            restore_documents_in(data.path(), &backup, None).unwrap(),
            Some(1)
        );
        let link = data.path().join("documents");
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(real.join("1/v1.pdf")).unwrap(), b"fresh");
        assert!(!real.join("9").exists());
    }
}
