//! Writes an invoice preview to a private temp file, for `open_external` to
//! hand to the system's own viewer where the webview has no PDF renderer of
//! its own.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tempfile::Builder;

/// The one preview temp file that exists at a time.
///
/// Recording a new path deletes whichever one it replaces, so at most one
/// preview sits in the world-readable temp directory at once. The current
/// path is also the only one `open_external` is allowed to hand to the
/// system opener.
#[derive(Default)]
pub struct PreviewPaths(Mutex<Option<PathBuf>>);

impl PreviewPaths {
    /// Whether `path` is the file `write_temp_pdf` most recently produced.
    pub fn contains(&self, path: &Path) -> bool {
        self.0.lock().expect("preview path lock").as_deref() == Some(path)
    }

    pub(crate) fn record(&self, path: PathBuf) {
        let previous = self.0.lock().expect("preview path lock").replace(path);
        if let Some(previous) = previous {
            let _ = std::fs::remove_file(previous);
        }
    }
}

fn write_to(paths: &PreviewPaths, name: &str, bytes: &[u8]) -> Result<String, String> {
    let mut file = Builder::new()
        .prefix(&format!("{name}-"))
        .suffix(".pdf")
        .tempfile()
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    let (_file, path) = file.keep().map_err(|e| e.to_string())?;
    paths.record(path.clone());
    Ok(path.display().to_string())
}

/// Write `bytes` to a temp file named after `name`, and answer its path.
///
/// The file this call replaces is deleted; the one it writes is not, because
/// the external viewer it was opened in is still reading it after this
/// command returns. The system temp directory is where the OS expects to
/// reclaim whatever is left.
#[tauri::command]
pub async fn write_temp_pdf(
    paths: tauri::State<'_, PreviewPaths>,
    name: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    write_to(&paths, &name, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_writes_the_bytes_and_answers_a_path_that_holds_them() {
        let paths = PreviewPaths::default();
        let path = write_to(&paths, "invoice-1251", b"%PDF-1.4").expect("write the temp pdf");

        let written = std::fs::read(&path).expect("read the file back");
        assert_eq!(written, b"%PDF-1.4");
        assert!(path.ends_with(".pdf"), "expected a .pdf path, got {path}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn it_lands_in_the_system_temp_directory() {
        let paths = PreviewPaths::default();
        let path = write_to(&paths, "invoice-1251", b"%PDF-1.4").expect("write the temp pdf");

        assert!(
            Path::new(&path).starts_with(std::env::temp_dir()),
            "expected {path} to live under {}",
            std::env::temp_dir().display()
        );

        std::fs::remove_file(&path).ok();
    }

    #[cfg(unix)]
    #[test]
    fn it_is_not_world_or_group_readable_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let paths = PreviewPaths::default();
        let path = write_to(&paths, "invoice-1251", b"%PDF-1.4").expect("write the temp pdf");

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

    #[test]
    fn it_records_the_path_it_writes_and_nothing_else() {
        let paths = PreviewPaths::default();
        let path = write_to(&paths, "invoice-1251", b"%PDF-1.4").expect("write the temp pdf");

        assert!(paths.contains(Path::new(&path)));
        assert!(!paths.contains(Path::new("/etc/passwd")));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn it_deletes_the_previous_preview_when_writing_a_new_one() {
        let paths = PreviewPaths::default();
        let first = write_to(&paths, "invoice-1251", b"%PDF-1.4").expect("write the first pdf");
        assert!(Path::new(&first).exists());

        let second = write_to(&paths, "invoice-1300", b"%PDF-1.5").expect("write the second pdf");

        assert!(
            !Path::new(&first).exists(),
            "expected the previous preview at {first} to be gone"
        );
        assert!(Path::new(&second).exists());

        std::fs::remove_file(&second).ok();
    }
}
