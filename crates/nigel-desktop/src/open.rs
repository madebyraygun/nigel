//! Hands a file to the system's own viewer for it.

use tauri_plugin_opener::OpenerExt;

/// Hand a file to whatever the system uses for its type.
#[tauri::command]
pub async fn open_external(app: tauri::AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}
