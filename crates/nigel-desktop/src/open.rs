//! Hands a file to the system's own viewer for it.

use tauri_plugin_opener::OpenerExt;

use crate::preview::PreviewPaths;

fn ensure_known(paths: &PreviewPaths, path: &str) -> Result<(), String> {
    if paths.contains(std::path::Path::new(path)) {
        Ok(())
    } else {
        Err(format!(
            "refusing to open a path nigel did not create: {path}"
        ))
    }
}

/// Hand a file to whatever the system uses for its type.
///
/// Refuses anything `write_temp_pdf` did not itself produce: the opener
/// plugin's own API bypasses its scope config, and `xdg-open`/`start` accept
/// a URL as readily as a path, so an unvalidated argument here would let a
/// caller open anything reachable on the machine.
#[tauri::command]
pub async fn open_external(
    app: tauri::AppHandle,
    paths: tauri::State<'_, PreviewPaths>,
    path: String,
) -> Result<(), String> {
    ensure_known(&paths, &path)?;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn refuses_a_path_write_temp_pdf_never_produced() {
        let paths = PreviewPaths::default();
        paths.record(PathBuf::from("/tmp/nigel-preview-real.pdf"));

        assert!(ensure_known(&paths, "/tmp/nigel-preview-real.pdf").is_ok());
        assert!(ensure_known(&paths, "/etc/passwd").is_err());
    }
}
