//! Where the desktop shell finds its database.

use std::path::PathBuf;

/// The database the desktop shell opens — the same one the CLI opens.
pub fn database_path() -> PathBuf {
    nigel_core::settings::get_data_dir().join("nigel.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_opens_the_same_database_the_cli_does() {
        // A desktop app pointing at its own database would silently show the user
        // an empty set of books.
        assert_eq!(
            database_path(),
            nigel_core::settings::get_data_dir().join("nigel.db")
        );
    }
}
