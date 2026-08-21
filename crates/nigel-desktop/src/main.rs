//! The desktop shell: nigel's SPA and JSON API over one custom URI scheme.

use std::sync::Arc;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use nigel_desktop::{db, imports, save, scheme_url, transport, window_state, SCHEME};

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
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                remember_geometry(
                    window.scale_factor(),
                    window.inner_size(),
                    window.outer_position(),
                );
                // Closing hides on macOS: the app survives its last window,
                // and Reopen brings the same window back with its state
                // intact. Elsewhere the close proceeds and the app exits
                // with its window.
                #[cfg(target_os = "macos")]
                {
                    api.prevent_close();
                    let _ = window.hide();
                }
                #[cfg(not(target_os = "macos"))]
                let _ = api;
            }
        })
        .setup(|app| {
            build_main_window(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("build nigel desktop")
        .run(|app, event| match event {
            // The app outlives its last window on macOS. An explicit exit —
            // Quit carries a code — still exits.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } => {
                api.prevent_exit();
            }
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } => {
                if !has_visible_windows {
                    match app.get_webview_window("main") {
                        Some(window) => {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        // The window is genuinely gone (a webview crash),
                        // not hidden: rebuild it.
                        None => {
                            let _ = build_main_window(app);
                        }
                    }
                }
            }
            tauri::RunEvent::ExitRequested { .. } => {
                if let Some(window) = app.get_webview_window("main") {
                    remember_geometry(
                        window.scale_factor(),
                        window.inner_size(),
                        window.outer_position(),
                    );
                }
            }
            _ => {}
        });
}

/// The main window, restored to where its last close left it.
fn build_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let restored = window_state::load_from(&window_state::state_path())
        .map(|saved| window_state::clamp_restore(saved, &monitor_areas(app)));

    let builder = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::CustomProtocol(scheme_url().parse().expect("scheme url")),
    )
    .title("Nigel")
    .min_inner_size(900.0, 700.0);

    let builder = match restored {
        Some(geometry) => builder
            .inner_size(geometry.width, geometry.height)
            .position(geometry.x, geometry.y),
        None => builder.inner_size(1200.0, 820.0),
    };

    builder.build()?;
    Ok(())
}

/// Every monitor's usable region, in the logical units the state file keeps.
fn monitor_areas(app: &tauri::AppHandle) -> Vec<window_state::MonitorArea> {
    let monitors = app.available_monitors().unwrap_or_default();
    monitors
        .iter()
        .map(|monitor| {
            let scale = monitor.scale_factor();
            let area = monitor.work_area();
            let position = area.position.to_logical::<f64>(scale);
            let size = area.size.to_logical::<f64>(scale);
            window_state::MonitorArea {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

/// Write the window's geometry to the state file; best-effort by
/// [`window_state`]'s contract.
///
/// Takes the readings rather than a window: the close path holds a
/// `Window` and the exit path a `WebviewWindow`, which share these
/// accessors but no trait.
fn remember_geometry(
    scale: tauri::Result<f64>,
    size: tauri::Result<tauri::PhysicalSize<u32>>,
    position: tauri::Result<tauri::PhysicalPosition<i32>>,
) {
    let (Ok(scale), Ok(size), Ok(position)) = (scale, size, position) else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    let position = position.to_logical::<f64>(scale);
    window_state::save_to(
        &window_state::state_path(),
        window_state::WindowGeometry {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
        },
    );
}
