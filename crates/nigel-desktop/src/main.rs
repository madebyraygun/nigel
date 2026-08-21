//! The desktop shell: nigel's SPA and JSON API over one custom URI scheme.

use std::sync::Arc;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use nigel_desktop::{chrome, db, imports, save, scheme_url, transport, window_state, SCHEME};

fn main() {
    let state = nigel_core::server::state::AppState::new(
        db::database_path(),
        nigel_core::server::auth::generate_token(),
    );
    let router = nigel_core::server::build_desktop_router(state, nigel_desktop::trusted_origins());
    let runtime = Arc::new(
        tokio::runtime::Runtime::new().expect("build tokio runtime for the scheme handler"),
    );
    let saver = Arc::new(window_state::GeometrySaver::spawn(
        window_state::state_path(),
    ));
    let exit_saver = Arc::clone(&saver);
    #[cfg(target_os = "macos")]
    let fullscreen_close = FullscreenClose::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            save::save_export,
            imports::stage_import,
            imports::pick_import_file,
            chrome::frontend_ready,
            chrome::set_chrome_background
        ])
        .register_asynchronous_uri_scheme_protocol(SCHEME, move |_ctx, request, responder| {
            let router = router.clone();
            let runtime = runtime.clone();
            runtime.spawn(async move {
                responder.respond(transport::answer(router, request).await);
            });
        })
        .on_window_event(move |window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                // Geometry is saved as the window moves — macOS quit is
                // AppKit's `terminate:`, which raises no window event and
                // no ExitRequested, so waiting for close would miss the
                // most common quit path entirely.
                tauri::WindowEvent::Moved(_) => observe(&saver, window),
                tauri::WindowEvent::Resized(_) => {
                    observe(&saver, window);
                    #[cfg(target_os = "macos")]
                    fullscreen_close.resized();
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    observe(&saver, window);
                    saver.save_now();
                    // Closing hides on macOS: the app survives its last
                    // window, and Reopen brings the same window back with
                    // its state intact. Elsewhere the close proceeds and
                    // the app exits with its window.
                    #[cfg(target_os = "macos")]
                    {
                        api.prevent_close();
                        if window.is_fullscreen().unwrap_or(false) {
                            fullscreen_close.begin(window);
                        } else {
                            let _ = window.hide();
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    let _ = api;
                }
                _ => {}
            }
        })
        .setup(|app| {
            build_main_window(app.handle())?;
            // A wedged frontend must not leave an invisible process: whatever
            // has not shown four seconds after setup shows as it is.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(4));
                if let Some(window) = handle.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("build nigel desktop")
        .run(move |app, event| {
            #[cfg(not(target_os = "macos"))]
            let _ = app;
            match event {
                // Loop teardown is the one signal every quit path shares —
                // macOS `terminate:` included — so the settle window can
                // never eat the last observation.
                tauri::RunEvent::Exit => exit_saver.save_now(),
                // The app outlives its last window on macOS: the only
                // no-code exit request is the last window closing, and it
                // is prevented. Quit bypasses this arm entirely — see the
                // save-on-move note above.
                #[cfg(target_os = "macos")]
                tauri::RunEvent::ExitRequested {
                    code: None, api, ..
                } => {
                    api.prevent_exit();
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen {
                    has_visible_windows: false,
                    ..
                } => {
                    match app.get_webview_window("main") {
                        Some(window) => {
                            // A hidden window shows; a minimized one
                            // ignores show and focus until it is
                            // deminiaturized.
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        // The window is genuinely gone (a webview
                        // crash), not hidden: rebuild it. A windowless
                        // app that cannot rebuild would strand the
                        // user, so that failure exits.
                        None => {
                            if let Err(error) = build_main_window(app) {
                                eprintln!("nigel: could not rebuild the main window: {error}");
                                app.exit(1);
                            }
                        }
                    }
                }
                _ => {}
            }
        });
}

/// The main window, restored to where its last close left it.
///
/// Restore is planned in the platform's one coherent coordinate space and
/// applied with `set_position` — the same frame-top-left convention the
/// saved reading used — because the builder's `position` means the content
/// origin on macOS and would land the frame a title bar too high. The window
/// is built hidden and placed while hidden; it shows once the SPA signals it
/// has painted (`chrome::frontend_ready`), so it never flashes at the
/// default spot or as a blank canvas first.
fn build_main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let saved = window_state::load_from(&window_state::state_path());
    let monitors = monitor_areas(app);
    let plan = window_state::plan_launch(
        saved.as_ref(),
        &monitors,
        saved.as_ref().map_or(1.0, restore_space_scale),
    );

    let window = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::CustomProtocol(scheme_url().parse().expect("scheme url")),
    )
    .title("Nigel")
    .visible(false)
    .min_inner_size(window_state::MIN_WIDTH, window_state::MIN_HEIGHT)
    .inner_size(plan.inner_width, plan.inner_height)
    .build()?;

    if let Some((x, y)) = plan.frame {
        let _ = window.set_position(frame_position(x, y));
    }
    if plan.maximized {
        let _ = window.maximize();
    }
    // The OS theme is the best signal before the SPA's first frame; the
    // frontend refines this through set_chrome_background once it has
    // resolved any stored override.
    let theme = window.theme().unwrap_or(tauri::Theme::Light);
    let _ = window.set_background_color(Some(chrome::background_for(theme)));
    Ok(())
}

/// A close that arrives while the window is fullscreen. Hiding it there
/// would strand an empty fullscreen Space, so the window leaves fullscreen
/// first and hides once the exit transition's resizes have settled — a hide
/// mid-transition strands the Space just the same. Tauri raises no
/// transition-finished event, so quiet stands in for done, bounded in case
/// no resize arrives at all.
#[cfg(target_os = "macos")]
#[derive(Default)]
struct FullscreenClose(std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>);

#[cfg(target_os = "macos")]
impl FullscreenClose {
    const FIRST_RESIZE: std::time::Duration = std::time::Duration::from_secs(2);
    const SETTLE: std::time::Duration = std::time::Duration::from_millis(500);

    fn begin(&self, window: &tauri::Window) {
        let (resized, resizes) = std::sync::mpsc::channel();
        *self.0.lock().expect("fullscreen close lock") = Some(resized);
        let hiding = window.clone();
        std::thread::spawn(move || {
            let _ = resizes.recv_timeout(Self::FIRST_RESIZE);
            while resizes.recv_timeout(Self::SETTLE).is_ok() {}
            let _ = hiding.hide();
        });
        let _ = window.set_fullscreen(false);
    }

    fn resized(&self) {
        if let Some(resized) = self.0.lock().expect("fullscreen close lock").as_ref() {
            let _ = resized.send(());
        }
    }
}

/// The clamp space for a restore; see [`window_state::plan_restore`].
#[cfg(target_os = "macos")]
fn restore_space_scale(_geometry: &window_state::WindowGeometry) -> f64 {
    1.0
}
#[cfg(not(target_os = "macos"))]
fn restore_space_scale(geometry: &window_state::WindowGeometry) -> f64 {
    geometry.scale
}

/// A planned frame origin as the position type its clamp space implies.
#[cfg(target_os = "macos")]
fn frame_position(x: f64, y: f64) -> tauri::Position {
    tauri::Position::Logical(tauri::LogicalPosition::new(x, y))
}
#[cfg(not(target_os = "macos"))]
fn frame_position(x: f64, y: f64) -> tauri::Position {
    tauri::Position::Physical(tauri::PhysicalPosition::new(
        x.round() as i32,
        y.round() as i32,
    ))
}

/// Every monitor's usable region, in the space restore math runs in:
/// logical points on macOS — the one space that stays coherent across
/// mixed-scale monitors there — and physical pixels everywhere else, where
/// the opposite is true.
fn monitor_areas(app: &tauri::AppHandle) -> Vec<window_state::MonitorArea> {
    let monitors = app.available_monitors().unwrap_or_default();
    monitors.iter().map(monitor_area).collect()
}

#[cfg(target_os = "macos")]
fn monitor_area(monitor: &tauri::window::Monitor) -> window_state::MonitorArea {
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
}
#[cfg(not(target_os = "macos"))]
fn monitor_area(monitor: &tauri::window::Monitor) -> window_state::MonitorArea {
    let area = monitor.work_area();
    window_state::MonitorArea {
        x: f64::from(area.position.x),
        y: f64::from(area.position.y),
        width: f64::from(area.size.width),
        height: f64::from(area.size.height),
    }
}

/// Feed the window's current state to the saver: a normal frame verbatim, a
/// maximized window as a flag over the frame it will return to, a minimized
/// or fullscreen window not at all — minimized readings are sentinels
/// (-32000 on Windows), not geometry.
fn observe(saver: &window_state::GeometrySaver, window: &tauri::Window) {
    if window.is_minimized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
        return;
    }
    if window.is_maximized().unwrap_or(false) {
        saver.observe_maximized();
        return;
    }
    if let Some(geometry) = read_geometry(window) {
        saver.observe_frame(geometry);
    }
}

fn read_geometry(window: &tauri::Window) -> Option<window_state::WindowGeometry> {
    let scale = window.scale_factor().ok()?;
    let inner = window.inner_size().ok()?.to_logical::<f64>(scale);
    let outer = window.outer_size().ok()?.to_logical::<f64>(scale);
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    Some(window_state::WindowGeometry {
        width: inner.width,
        height: inner.height,
        x: position.x,
        y: position.y,
        extra_width: (outer.width - inner.width).max(0.0),
        extra_height: (outer.height - inner.height).max(0.0),
        scale,
        maximized: false,
    })
}
