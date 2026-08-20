//! A file staged from disk and a file uploaded through the API are the same
//! thing downstream. This drives the desktop router over the staged id and
//! asserts preview and confirm behave exactly as the browser pipeline's do.

use nigel_core::server::state::AppState;
use nigel_core::server::{build_desktop_router, testutil, uploads};

use nigel_desktop::imports::stage_file;

fn router_over(db_path: &std::path::Path) -> axum::Router {
    let token = nigel_core::server::auth::generate_token();
    let state = AppState::new(db_path.to_path_buf(), token);
    build_desktop_router(state, nigel_desktop::trusted_origins())
}

fn post_json(path: &str, body: &str) -> tauri::http::Request<Vec<u8>> {
    tauri::http::Request::builder()
        .method("POST")
        .uri(format!(
            "{}{}",
            nigel_desktop::scheme_url(),
            path.trim_start_matches('/')
        ))
        .header(tauri::http::header::HOST, nigel_desktop::trusted_host())
        .header(tauri::http::header::CONTENT_TYPE, "application/json")
        .body(body.as_bytes().to_vec())
        .expect("build scheme request")
}

/// Three rows a built-in importer can read, in Bank of America's checking
/// layout — the format `seeded_db`'s account is set up for.
fn statement(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("cedar-april-2025.csv");
    std::fs::write(
        &path,
        "Date,Description,Amount,Running Bal.\n\
         04/01/2025,CEDAR SYSTEMS INVOICE 002,1250.00,0.00\n\
         04/03/2025,JUNIPER LABS HOSTING,-84.50,0.00\n\
         04/09/2025,HARBOR AND VALE RETAINER,-119.00,0.00\n",
    )
    .expect("write statement");
    path
}

#[tokio::test]
async fn a_staged_file_previews_and_confirms_like_an_uploaded_one() {
    let (dir, db_path) = testutil::seeded_db();
    let source = statement(dir.path());

    let staged = stage_file(&source, &uploads::uploads_dir(&db_path)).expect("stage");
    assert_eq!(staged.filename, "cedar-april-2025.csv");
    assert_eq!(staged.path, source.display().to_string());

    let router = router_over(&db_path);
    let body = format!(
        r#"{{"uploadId":"{}","account":"BofA Checking"}}"#,
        staged.upload_id
    );

    let response =
        nigel_desktop::transport::answer(router.clone(), post_json("/api/imports/preview", &body))
            .await;
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    let preview: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(preview["format"], "bofa_checking");
    assert_eq!(preview["imported"], 3);
    assert_eq!(preview["duplicateFile"], false);
    assert_eq!(preview["importId"], serde_json::Value::Null);

    let response =
        nigel_desktop::transport::answer(router, post_json("/api/imports/confirm", &body)).await;
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    let confirmed: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(confirmed["imported"], 3);
    assert!(confirmed["importId"].is_i64(), "{confirmed}");

    // Confirm consumes the spooled file whichever way it got there.
    assert!(
        uploads::resolve(&uploads::uploads_dir(&db_path), &staged.upload_id).is_none(),
        "the staged file outlived its confirm"
    );
}

#[tokio::test]
async fn a_staged_id_the_spool_has_forgotten_is_the_upload_expired_404() {
    // The screen's re-stage-and-retry hangs off this exact answer, so it is
    // worth pinning that the staged path produces it too.
    let (dir, db_path) = testutil::seeded_db();
    let source = statement(dir.path());

    let staged = stage_file(&source, &uploads::uploads_dir(&db_path)).expect("stage");
    uploads::delete(&uploads::uploads_dir(&db_path), &staged.upload_id);

    let body = format!(
        r#"{{"uploadId":"{}","account":"BofA Checking"}}"#,
        staged.upload_id
    );
    let response = nigel_desktop::transport::answer(
        router_over(&db_path),
        post_json("/api/imports/preview", &body),
    )
    .await;

    assert_eq!(response.status(), 404);
    let error: serde_json::Value = serde_json::from_slice(response.body()).expect("json");
    assert_eq!(error["error"]["details"]["reason"], "upload_not_found");
}

#[test]
fn the_shell_grants_itself_only_the_event_permissions_drag_and_drop_needs() {
    // `plugin:event|listen` is ACL-checked on every call, unlike an app
    // command, so without a capability the page's drag-drop subscription is
    // rejected and a drop goes nowhere. The list is pinned rather than merely
    // non-empty: `core:default` or a filesystem permission here would hand the
    // page far more than the four events the import screen listens for.
    let source = std::fs::read_to_string("capabilities/default.json").expect("read the capability");
    let capability: serde_json::Value = serde_json::from_str(&source).expect("json");

    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert_eq!(
        capability["permissions"],
        serde_json::json!(["core:event:allow-listen", "core:event:allow-unlisten"])
    );
}

#[test]
fn both_staging_commands_are_reachable_from_the_page() {
    // `generate_handler!` is a macro over a literal list, so a command written
    // and never registered compiles, ships, and answers "not allowed" the
    // first time anyone drops a file on the window.
    let main = std::fs::read_to_string("src/main.rs").expect("read main.rs");

    for command in ["imports::stage_import", "imports::pick_import_file"] {
        assert!(
            main.contains(command),
            "{command} is not in the invoke handler"
        );
    }
}
