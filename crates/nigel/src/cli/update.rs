pub use nigel_core::updater::*;

use std::io::Write;

use nigel_core::error::{NigelError, Result};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Download the binary from `url` and replace the current executable.
fn download_and_install(url: &str) -> Result<()> {
    println!("Downloading...");
    let bytes = download_release(url)?;

    // Write to a temp file with a unique name, then atomically replace
    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("nigel-update-{}", std::process::id()));
    std::fs::write(&tmp_path, &bytes)?;

    // Set executable permission on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))?;
    }

    println!("Installing ({} bytes)...", bytes.len());
    self_replace::self_replace(&tmp_path)
        .map_err(|e| NigelError::Other(format!("Failed to replace binary: {e}")))?;

    // Clean up temp file (best effort)
    let _ = std::fs::remove_file(&tmp_path);

    Ok(())
}

/// The `nigel update` CLI command.
pub fn run() -> Result<()> {
    println!("Checking for updates...");
    match check_for_update() {
        Ok(Some(info)) => {
            print!(
                "Nigel v{} is available (current: v{CURRENT_VERSION}). Install? [Y/n] ",
                info.version
            );
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let input = input.trim().to_lowercase();
            if input.is_empty() || input == "y" || input == "yes" {
                download_and_install(&info.download_url)?;
                println!(
                    "Updated to v{}. Restart nigel to use the new version.",
                    info.version
                );
            } else {
                println!("Update cancelled.");
            }
        }
        Ok(None) => {
            println!("You're on the latest version (v{CURRENT_VERSION}).");
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(())
}
