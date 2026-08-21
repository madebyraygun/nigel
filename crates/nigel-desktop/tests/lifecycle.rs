//! The lifecycle promises: macOS keeps running without a window and hides on
//! close; everywhere else, close still exits. Read off the source rather than
//! a running app — building a window needs a display server, which CI has
//! not got, and the behaviors under test are macOS-only anyway.

use std::fs;

fn main_rs() -> String {
    fs::read_to_string("src/main.rs").expect("read main.rs")
}

/// The attribute directly above the first line containing `needle`, looking
/// past comments, blank lines, and a bare `{`. Anchoring on the guarded
/// line itself is what lets these tests refuse: a `cfg` elsewhere in the
/// file cannot stand in for the one that was deleted here.
fn attribute_directly_above(source: &str, needle: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let at = lines.iter().position(|line| line.contains(needle))?;
    lines[..at]
        .iter()
        .rev()
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !line.starts_with("//") && *line != "{")
        .filter(|line| line.starts_with("#["))
        .map(str::to_string)
}

const MACOS_CFG: &str = "#[cfg(target_os = \"macos\")]";

#[test]
fn closing_hides_rather_than_exits_on_macos_only() {
    let main = main_rs();

    assert_eq!(
        attribute_directly_above(&main, "api.prevent_close()").as_deref(),
        Some(MACOS_CFG),
        "hide-on-close is not confined to macOS"
    );
    let prevent_at = main.find("api.prevent_close()").expect("prevent_close");
    let hide_at = main.find("window.hide()").expect("the window hides");
    assert!(
        prevent_at < hide_at,
        "the close is not prevented before the hide"
    );
    assert!(
        main.contains("#[cfg(not(target_os = \"macos\"))]"),
        "the non-macOS close path is no longer explicit"
    );
}

#[test]
fn the_app_outlives_its_last_window_on_macos() {
    let main = main_rs();

    // The cfg must sit on the keep-alive arm itself — a macos cfg anywhere
    // earlier in the file must not satisfy this. Without it, Windows and
    // Linux would prevent their windowless exit and linger headless.
    assert_eq!(
        attribute_directly_above(&main, "tauri::RunEvent::ExitRequested {").as_deref(),
        Some(MACOS_CFG),
        "the keep-alive arm is not confined to macOS"
    );

    let arm_at = main
        .find("tauri::RunEvent::ExitRequested {")
        .expect("an exit-request arm");
    let code_at = main
        .find("code: None")
        .expect("the no-code exit request is matched");
    let prevent_at = main
        .find("api.prevent_exit()")
        .expect("the exit is prevented");
    assert!(
        arm_at < code_at && code_at < prevent_at,
        "prevent_exit is not tied to the windowless exit request"
    );
}

#[test]
fn the_dock_reopens_the_window() {
    let main = main_rs();

    assert_eq!(
        attribute_directly_above(&main, "tauri::RunEvent::Reopen").as_deref(),
        Some(MACOS_CFG),
        "the Reopen arm is not confined to macOS"
    );

    let reopen = &main[main.find("tauri::RunEvent::Reopen").expect("reopen arm")..];
    let unminimize_at = reopen
        .find("window.unminimize()")
        .expect("reopen deminiaturizes: show and focus no-op on a minimized window");
    let show_at = reopen
        .find("window.show()")
        .expect("reopen shows the hidden window");
    let rebuild_at = reopen
        .find("build_main_window(app)")
        .expect("reopen rebuilds a destroyed window");
    assert!(
        unminimize_at < show_at && show_at < rebuild_at,
        "expected unminimize, then show, with rebuild as the fallback"
    );
    assert!(
        reopen.contains("app.exit(1)"),
        "a failed rebuild must exit rather than strand a windowless app"
    );
}

#[test]
fn geometry_is_saved_where_it_is_restored_from() {
    let main = main_rs();

    // One saver over one path: spawned on state_path, restored from
    // state_path, and nothing else reads it — the file cannot fork.
    let spawn_at = main
        .find("window_state::GeometrySaver::spawn(")
        .expect("a geometry saver is spawned");
    assert!(
        main[spawn_at..spawn_at + 120].contains("window_state::state_path()"),
        "the saver is not spawned on state_path"
    );
    assert!(main.contains("window_state::load_from(&window_state::state_path())"));
    assert_eq!(
        main.matches("window_state::state_path()").count(),
        2,
        "the saver and the restore should be the only state_path users in the shell"
    );

    // Quit raises no window event on macOS, so geometry must be observed
    // as the window moves and resizes, not only at close.
    assert!(
        main.contains("tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)"),
        "geometry is no longer observed as the window moves"
    );

    // Each flush is asserted inside its own arm's span, so neither can
    // stand in for the other.
    let close_at = main
        .find("tauri::WindowEvent::CloseRequested")
        .expect("a close arm");
    let setup_at = main.find(".setup(").expect("a setup hook");
    assert!(
        main[close_at..setup_at].contains("saver.save_now()"),
        "the close arm no longer flushes the saver"
    );

    // The settle window must not be able to eat the final move: loop
    // teardown (RunEvent::Exit) is the one signal every quit path reaches,
    // macOS terminate: included, and it flushes.
    let run_at = main.find(".run(").expect("the run loop");
    let exit_at = main[run_at..]
        .find("tauri::RunEvent::Exit => ")
        .map(|at| run_at + at)
        .expect("the run loop no longer handles RunEvent::Exit");
    assert!(
        main[exit_at..].starts_with("tauri::RunEvent::Exit => exit_saver.save_now()"),
        "loop teardown no longer flushes the saver"
    );
}
