//! Spooling a file the user already has on disk into the same place a browser
//! upload lands, so the rest of the import pipeline cannot tell them apart.

use std::path::Path;

use tauri_plugin_dialog::DialogExt;

use nigel_core::server::uploads;

use crate::db;

/// Staging is `nigel-core`'s, shared with the Swift shell's `nigel-ffi`.
pub use nigel_core::server::uploads::{stage_file, StagedUpload};

/// Spool a file the user dropped onto the window.
///
/// Async for `stage_file`'s benefit rather than the dialog's: reading and
/// writing a statement is blocking work, and an async `#[tauri::command]` runs
/// it on the async runtime instead of the main thread.
#[tauri::command]
pub async fn stage_import(path: String) -> Result<StagedUpload, String> {
    stage_file(
        Path::new(&path),
        &uploads::uploads_dir(&db::database_path().map_err(|e| e.to_string())?),
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
        .add_filter(
            "Statements",
            uploads::UploadArea::Statement.allowed_extensions(),
        )
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let db_path = db::database_path().map_err(|e| e.to_string())?;
    stage_file(&path, &uploads::uploads_dir(&db_path)).map(Some)
}
