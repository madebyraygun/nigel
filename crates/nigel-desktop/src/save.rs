//! Writes an export's bytes to wherever the user picks in a save dialog.

use tauri_plugin_dialog::DialogExt;

/// Write exported bytes wherever the user chooses.
///
/// `Ok(None)` is a cancelled dialog, which is a normal outcome and not an
/// error: the user changed their mind.
///
/// Async rather than sync: `blocking_save_file` must not run on the main
/// thread, and an async `#[tauri::command]` runs on the async runtime
/// instead.
#[tauri::command]
pub async fn save_export(
    app: tauri::AppHandle,
    name: String,
    bytes: Vec<u8>,
) -> Result<Option<String>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(&name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, &bytes).map_err(|e| format!("Couldn't save {name}: {e}"))?;
    Ok(Some(path.display().to_string()))
}
