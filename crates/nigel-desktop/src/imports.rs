//! Spooling a file the user already has on disk into the same place a browser
//! upload lands, so the rest of the import pipeline cannot tell them apart.

use std::path::Path;

use serde::Serialize;

use nigel_core::server::uploads;

/// A file the desktop shell has spooled.
///
/// `uploadId`, `filename` and `size` are `POST /api/imports/upload`'s answer,
/// field for field. `path` is the one thing a native client knows and a
/// browser cannot: where the file came from, so an expired spool is re-staged
/// without asking the user for it again.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedUpload {
    pub upload_id: String,
    pub filename: String,
    pub size: u64,
    pub path: String,
}

/// Spool a file that is already on this machine, and answer with the same
/// `uploadId` a browser upload would have produced.
///
/// The checks are ordered by what they cost: the name is decided without
/// touching the file, the size is decided from the metadata, and the bytes are
/// read only once both have passed.
pub fn stage_file(path: &Path, uploads_dir: &Path) -> Result<StagedUpload, String> {
    let raw_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let filename = uploads::sanitize_filename(raw_name)?;

    let metadata = std::fs::metadata(path).map_err(|e| read_error(path, &e))?;
    if metadata.len() > uploads::MAX_UPLOAD_BYTES as u64 {
        let limit = uploads::MAX_UPLOAD_BYTES / (1024 * 1024);
        return Err(format!("That file is over the {limit} MB limit."));
    }

    let bytes = std::fs::read(path).map_err(|e| read_error(path, &e))?;

    // The upload route sweeps before every store; the desktop path is the same
    // spool, so it owes the same sweep.
    uploads::purge_stale(uploads_dir, uploads::MAX_AGE);
    let stored = uploads::store(uploads_dir, &filename, &bytes).map_err(|e| e.to_string())?;

    Ok(StagedUpload {
        upload_id: stored.id,
        filename: stored.filename,
        size: stored.size,
        path: path.display().to_string(),
    })
}

fn read_error(path: &Path, error: &std::io::Error) -> String {
    format!("Couldn't read {}: {error}", path.display())
}

use tauri_plugin_dialog::DialogExt;

use crate::db;

/// Spool a file the user dropped onto the window.
///
/// Async for `stage_file`'s benefit rather than the dialog's: reading and
/// writing a statement is blocking work, and an async `#[tauri::command]` runs
/// it on the async runtime instead of the main thread.
#[tauri::command]
pub async fn stage_import(path: String) -> Result<StagedUpload, String> {
    stage_file(
        Path::new(&path),
        &uploads::uploads_dir(&db::database_path()),
    )
}

/// Open a native file dialog filtered to what the importers read, and spool
/// whatever comes back.
///
/// `Ok(None)` is a cancelled dialog, which is a normal outcome and not an
/// error: the user changed their mind.
#[tauri::command]
pub async fn pick_import_file(app: tauri::AppHandle) -> Result<Option<StagedUpload>, String> {
    let Some(picked) = app
        .dialog()
        .file()
        .add_filter("Statements", &uploads::ALLOWED_EXTENSIONS)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    stage_file(&path, &uploads::uploads_dir(&db::database_path())).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A temp directory plus the spool inside it, laid out the way
    /// `uploads_dir` lays it out beside a real database.
    fn workspace() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let spool = dir.path().join("tmp").join("uploads");
        (dir, spool)
    }

    fn statement(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(
            &path,
            "Date,Description,Amount,Running Bal.\n\
             04/01/2025,CEDAR SYSTEMS INVOICE 002,1250.00,0.00\n\
             04/03/2025,JUNIPER LABS HOSTING,-84.50,0.00\n",
        )
        .expect("write statement");
        path
    }

    #[test]
    fn a_statement_is_spooled_under_a_sanitized_name() {
        let (dir, spool) = workspace();
        let source = statement(dir.path(), "Cedar Systems April 2025.csv");

        let staged = stage_file(&source, &spool).expect("stage");

        // The spaces are what `sanitize_filename` reduces; the extension is
        // what the importers dispatch on and must survive untouched.
        assert_eq!(staged.filename, "Cedar_Systems_April_2025.csv");
        assert_eq!(staged.size, std::fs::metadata(&source).unwrap().len());
        assert_eq!(staged.path, source.display().to_string());
        assert_eq!(staged.upload_id.len(), 32);

        let spooled = spool.join(&staged.upload_id).join(&staged.filename);
        assert!(spooled.is_file(), "nothing at {}", spooled.display());
        assert_eq!(
            std::fs::read(&spooled).unwrap(),
            std::fs::read(&source).unwrap()
        );
    }

    #[test]
    fn a_file_type_no_importer_reads_is_refused_by_name() {
        let (dir, spool) = workspace();
        let source = dir.path().join("notes.txt");
        std::fs::write(&source, "not a statement").expect("write");

        let error = stage_file(&source, &spool).expect_err("refused");

        assert!(error.contains("notes.txt"), "{error}");
        for extension in uploads::ALLOWED_EXTENSIONS {
            assert!(error.contains(extension), "{error} omits {extension}");
        }
        assert!(
            !spool.exists(),
            "a refused file still made a spool directory"
        );
    }

    #[test]
    fn a_file_over_the_cap_is_refused_before_it_is_read() {
        let (dir, spool) = workspace();
        let source = dir.path().join("huge.csv");
        // Sparse rather than 25 MiB of real bytes: the check reads the length
        // from the metadata, which is the point of doing it before the read.
        let file = std::fs::File::create(&source).expect("create");
        file.set_len(uploads::MAX_UPLOAD_BYTES as u64 + 1)
            .expect("set_len");
        drop(file);

        let error = stage_file(&source, &spool).expect_err("refused");

        assert!(error.contains("25 MB"), "{error}");
        assert!(
            !spool.exists(),
            "an oversized file still made a spool directory"
        );
    }

    #[test]
    fn a_path_that_is_not_there_reports_the_os_error() {
        let (dir, spool) = workspace();
        let source = dir.path().join("gone.csv");

        let error = stage_file(&source, &spool).expect_err("refused");

        assert!(error.contains("gone.csv"), "{error}");
        assert!(
            error.to_lowercase().contains("no such file"),
            "the OS error did not survive: {error}"
        );
    }
}
