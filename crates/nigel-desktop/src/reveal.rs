//! Shows the books folder in the platform's file manager.

use std::path::Path;
use std::process::Command;

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
    command.arg(dir);
    command
}

/// Open the folder that holds the open books.
///
/// The folder comes from settings rather than from the page, so the command
/// cannot be pointed anywhere else. The file manager is spawned and left to
/// run: Explorer exits non-zero even when it opened the window, so its status
/// says nothing worth reporting.
#[tauri::command]
pub async fn reveal_data_dir() -> Result<(), String> {
    let dir = nigel_core::settings::get_data_dir();
    if !dir.is_dir() {
        return Err(format!("{} is not a folder.", dir.display()));
    }
    reveal_command(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't open {}: {e}", dir.display()))
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
}
