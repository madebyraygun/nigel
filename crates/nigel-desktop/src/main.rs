//! The desktop shell: nigel's SPA and JSON API over one custom URI scheme.

use std::sync::Arc;

use tauri::{WebviewUrl, WebviewWindowBuilder};

use nigel_desktop::{db, imports, menu, save, scheme_url, transport, SCHEME};

fn main() {
    let state = nigel_core::server::state::AppState::new(
        db::database_path(),
        nigel_core::server::auth::generate_token(),
    );
    let router = nigel_core::server::build_desktop_router(state, nigel_desktop::trusted_origins());
    let runtime = Arc::new(
        tokio::runtime::Runtime::new().expect("build tokio runtime for the scheme handler"),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(menu::build)
        .on_menu_event(menu::forward)
        .invoke_handler(tauri::generate_handler![
            save::save_export,
            imports::stage_import,
            imports::pick_import_file
        ])
        .register_asynchronous_uri_scheme_protocol(SCHEME, move |_ctx, request, responder| {
            let router = router.clone();
            let runtime = runtime.clone();
            runtime.spawn(async move {
                responder.respond(transport::answer(router, request).await);
            });
        })
        .setup(|app| {
            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::CustomProtocol(scheme_url().parse().expect("scheme url")),
            )
            .title("Nigel")
            .inner_size(1200.0, 820.0)
            .min_inner_size(900.0, 700.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run nigel desktop");
}
