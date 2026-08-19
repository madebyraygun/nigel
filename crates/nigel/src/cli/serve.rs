//! `nigel serve` — the dispatch seam for the web server.

use std::path::Path;

use nigel_core::error::Result;

/// Migrate a database that is already there; leave an absent one absent.
///
/// An encrypted file is skipped — `serve` is exempt from the stdin password
/// prompt, so it is still locked here and the unlock endpoint runs its
/// migrations once the key arrives. An absent one is left for the setup gate,
/// which is where a machine with no books belongs.
pub(crate) fn preflight(db_path: &Path) -> Result<()> {
    if !db_path.exists() || nigel_core::db::is_encrypted(db_path)? {
        return Ok(());
    }
    let conn = nigel_core::db::get_connection(db_path)?;
    nigel_core::db::init_db(&conn)
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
        nigel_core::db::set_db_password(None);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("nigel.db");

        preflight(&db_path).expect("preflight");

        assert!(!db_path.exists(), "preflight created a database");
    }

    #[test]
    fn preflight_migrates_an_existing_plaintext_database() {
        nigel_core::db::set_db_password(None);
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
}
