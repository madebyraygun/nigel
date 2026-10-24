//! Where the desktop shell finds its database.

use std::path::PathBuf;

/// The database the desktop shell opens — the same one the CLI opens — or the
/// reason settings.json cannot say where it is. The default directory never
/// stands in for an unreadable file: binding it would serve, and offer to set
/// up, a second set of books for the life of the process.
pub fn database_path() -> nigel_core::error::Result<PathBuf> {
    Ok(nigel_core::settings::try_data_dir()?.join("nigel.db"))
}

/// What the launch failure dialog says when [`database_path`] refuses.
pub fn launch_failure_message(err: &nigel_core::error::NigelError) -> String {
    format!("Nigel can't open your books.\n\n{err}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_opens_the_same_database_the_cli_does() {
        let _config = nigel_core::settings::TempConfigDir::new();
        // A desktop app pointing at its own database would silently show the user
        // an empty set of books.
        assert_eq!(
            database_path().expect("readable settings"),
            nigel_core::settings::get_data_dir().join("nigel.db")
        );
    }

    #[test]
    fn an_unreadable_settings_file_is_a_launch_failure_not_the_default_books() {
        let _config = nigel_core::settings::TempConfigDir::new();
        let dir = nigel_core::settings::config_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            "{\"data_dir\": \"/books/acme\",,}\n",
        )
        .unwrap();

        let err = database_path().expect_err("unreadable settings.json");
        let message = launch_failure_message(&err);
        assert!(
            message.contains("settings.json could not be read"),
            "{message}"
        );
    }
}
