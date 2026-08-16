//! `nigel serve` — the dispatch seam for the web server.

use nigel_core::error::Result;

/// Pre-flight and start. `serve` is exempt from the stdin password prompt, so
/// an encrypted database is still locked at this point and cannot be migrated;
/// the unlock endpoint runs migrations once the password arrives.
#[cfg(feature = "serve")]
pub fn run(port: u16, no_open: bool) -> Result<()> {
    let db_path = nigel_core::settings::get_data_dir().join("nigel.db");

    if !nigel_core::db::is_encrypted(&db_path)? {
        let conn = nigel_core::db::get_connection(&db_path)?;
        nigel_core::db::init_db(&conn)?;
    }

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
