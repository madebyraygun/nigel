//! Shows the books folder in the platform's file manager.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The path written the way a file manager accepts it.
///
/// A canonicalized Windows path carries the `\\?\` verbatim prefix, which
/// Explorer does not understand: it opens Documents instead. `\\?\UNC\server`
/// is a network share and goes back to `\\server`; any other verbatim path
/// loses the prefix. Paths without it are returned unchanged, so this is a
/// no-op everywhere but Windows.
pub fn file_manager_path(dir: &Path) -> PathBuf {
    let text = dir.to_string_lossy();
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{share}"))
    } else if let Some(local) = text.strip_prefix(r"\\?\") {
        PathBuf::from(local)
    } else {
        dir.to_path_buf()
    }
}

/// The file manager invocation that opens `dir`: Finder on macOS, Explorer on
/// Windows, and whatever `xdg-open` hands a folder to elsewhere.
pub fn reveal_command(dir: &Path) -> Command {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let mut command = Command::new(program);
    command.arg(file_manager_path(dir));
    command
}

/// Open the folder that holds the open books.
///
/// The folder comes from settings rather than from the page, so the command
/// cannot be pointed anywhere else. The file manager's exit status says
/// nothing worth reporting — Explorer exits non-zero even when it opened the
/// window — so the child is reaped on a thread of its own rather than
/// awaited, which is all that keeps `open` and `xdg-open` from lingering as
/// zombies.
#[tauri::command]
pub async fn reveal_data_dir() -> Result<(), String> {
    let dir = nigel_core::settings::try_data_dir().map_err(|e| e.to_string())?;
    if !dir.is_dir() {
        return Err(format!("{} is not a folder.", dir.display()));
    }
    let mut child = reveal_command(&dir)
        .spawn()
        .map_err(|e| format!("Couldn't open {}: {e}", dir.display()))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_opens_the_folder_it_was_given() {
        let dir = Path::new("/tmp/acme-books");
        let command = reveal_command(dir);
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, vec![dir.as_os_str()]);
    }

    #[test]
    fn the_command_is_the_platform_file_manager() {
        let command = reveal_command(Path::new("/tmp/acme-books"));
        let expected = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        assert_eq!(command.get_program(), expected);
    }

    /// An unreadable settings.json is reported, never the default folder.
    #[tokio::test]
    async fn reveal_refuses_when_settings_json_is_unreadable() {
        let _config = nigel_core::settings::TempConfigDir::new();
        let dir = nigel_core::settings::config_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("settings.json"),
            "{\"data_dir\": \"/books/acme\",,}\n",
        )
        .unwrap();

        let err = reveal_data_dir().await.expect_err("refused");
        assert!(err.contains("settings.json could not be read"), "{err}");
    }

    #[test]
    fn a_plain_path_is_left_alone() {
        assert_eq!(
            file_manager_path(Path::new("/tmp/acme-books/nigel")),
            PathBuf::from("/tmp/acme-books/nigel")
        );
    }

    #[test]
    fn the_verbatim_prefix_is_dropped() {
        assert_eq!(
            file_manager_path(Path::new(r"\\?\D:\Books\acme")),
            PathBuf::from(r"D:\Books\acme")
        );
    }

    #[test]
    fn a_verbatim_share_goes_back_to_unc() {
        assert_eq!(
            file_manager_path(Path::new(r"\\?\UNC\fileserver\books\acme")),
            PathBuf::from(r"\\fileserver\books\acme")
        );
    }

    #[cfg(windows)]
    #[test]
    fn explorer_is_handed_the_path_without_the_prefix() {
        let command = reveal_command(Path::new(r"\\?\D:\Books\acme"));
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, vec![std::ffi::OsStr::new(r"D:\Books\acme")]);
    }
}
