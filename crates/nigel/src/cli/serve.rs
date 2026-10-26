//! `nigel serve` — the dispatch seam for the web server.

use std::ffi::OsString;
use std::path::Path;

use nigel_core::error::Result;

/// Unlock encrypted books from `NIGEL_DB_PASSWORD_FILE` or `NIGEL_DB_PASSWORD`
/// when either is set, then migrate whatever this process can open; leave an
/// absent database absent.
///
/// With neither variable, encrypted books stay locked — local mode, unlocked
/// over loopback — and the unlock endpoint runs their migrations. An absent
/// database is left for the setup gate, which is where a machine with no books
/// belongs.
pub(crate) fn preflight(db_path: &Path) -> Result<()> {
    preflight_with(
        db_path,
        std::env::var_os("NIGEL_DB_PASSWORD"),
        std::env::var_os("NIGEL_DB_PASSWORD_FILE"),
    )
}

fn preflight_with(db_path: &Path, raw: Option<OsString>, file: Option<OsString>) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    if nigel_core::db::is_encrypted(db_path)? && !unlock(db_path, raw, file)? {
        return Ok(());
    }
    let conn = nigel_core::db::get_connection(db_path)?;
    nigel_core::db::init_db(&conn)
}

#[cfg(feature = "serve")]
fn unlock(db_path: &Path, raw: Option<OsString>, file: Option<OsString>) -> Result<bool> {
    nigel_core::server::boot::unlock_with(db_path, raw, file)
}

#[cfg(not(feature = "serve"))]
fn unlock(_db_path: &Path, _raw: Option<OsString>, _file: Option<OsString>) -> Result<bool> {
    Ok(false)
}

#[cfg(feature = "serve")]
pub fn run(port: u16, no_open: bool) -> Result<()> {
    preflight(&nigel_core::settings::get_data_dir().join("nigel.db"))?;
    nigel_core::server::run(port, no_open)
}

#[cfg(not(feature = "serve"))]
pub fn run(port: u16, no_open: bool) -> Result<()> {
    let _ = (port, no_open);
    Err(nigel_core::error::NigelError::Other(
        "`nigel serve` requires the 'serve' feature — build with `cargo build --features serve`"
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_leaves_an_absent_database_absent() {
        // A web-first user must reach setup rather than silently getting
        // default books nobody asked for.
        nigel_core::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");

        preflight(&db_path).expect("preflight");

        assert!(!db_path.exists(), "preflight created a database");
    }

    #[test]
    fn preflight_migrates_an_existing_plaintext_database() {
        nigel_core::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");
        let conn = nigel_core::db::open_connection(&db_path, None).expect("open");
        nigel_core::db::init_db(&conn).expect("init");
        drop(conn);

        preflight(&db_path).expect("preflight");

        let conn = nigel_core::db::open_connection(&db_path, None).expect("reopen");
        assert_eq!(
            nigel_core::db::get_profile(&conn),
            nigel_core::db::Profile::Business
        );
    }

    #[cfg(feature = "serve")]
    #[test]
    fn preflight_leaves_encrypted_books_locked_without_a_password_variable() {
        nigel_core::db::set_db_password_for_tests(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");
        let conn = nigel_core::db::open_connection(&db_path, None).expect("open");
        nigel_core::db::init_db(&conn).expect("init");
        drop(conn);
        nigel_core::password::encrypt_database(&db_path, "correct horse battery staple")
            .expect("encrypt");
        nigel_core::db::set_db_password_for_tests(None);

        preflight_with(&db_path, None, None).expect("preflight");
        assert!(nigel_core::db::get_db_password().is_none());

        preflight_with(&db_path, Some("correct horse battery staple".into()), None)
            .expect("preflight");
        assert!(nigel_core::db::get_db_password().is_some());
        nigel_core::db::set_db_password_for_tests(None);
    }
}
