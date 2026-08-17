//! Writes an invoice preview to a private temp file, for `open_external` to
//! hand to the system's own viewer where the webview has no PDF renderer of
//! its own.

use std::io::Write;

use tempfile::Builder;

/// Write `bytes` to a temp file named after `name`, and answer its path.
///
/// The system temp directory is where the OS expects to reclaim files like
/// this one — nothing here deletes it, because the external viewer the file
/// is opened in is still reading it after this command returns.
#[tauri::command]
pub async fn write_temp_pdf(name: String, bytes: Vec<u8>) -> Result<String, String> {
    let mut file = Builder::new()
        .prefix(&format!("{name}-"))
        .suffix(".pdf")
        .tempfile()
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    let (_file, path) = file.keep().map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_writes_the_bytes_and_answers_a_path_that_holds_them() {
        let path = write_temp_pdf("invoice-1251".to_string(), b"%PDF-1.4".to_vec())
            .await
            .expect("write the temp pdf");

        let written = std::fs::read(&path).expect("read the file back");
        assert_eq!(written, b"%PDF-1.4");
        assert!(path.ends_with(".pdf"), "expected a .pdf path, got {path}");

        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn it_lands_in_the_system_temp_directory() {
        let path = write_temp_pdf("invoice-1251".to_string(), b"%PDF-1.4".to_vec())
            .await
            .expect("write the temp pdf");

        assert!(
            std::path::Path::new(&path).starts_with(std::env::temp_dir()),
            "expected {path} to live under {}",
            std::env::temp_dir().display()
        );

        std::fs::remove_file(&path).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn it_is_not_world_or_group_readable_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let path = write_temp_pdf("invoice-1251".to_string(), b"%PDF-1.4".to_vec())
            .await
            .expect("write the temp pdf");

        let mode = std::fs::metadata(&path)
            .expect("stat the temp file")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "expected no group/other permission bits, got {mode:o}"
        );

        std::fs::remove_file(&path).ok();
    }
}
