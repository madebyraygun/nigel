//! The lifecycle promises: macOS keeps running without a window and hides on
//! close; everywhere else, close still exits. Read off the source rather than
//! a running app — building a window needs a display server, which CI has
//! not got, and the behaviors under test are macOS-only anyway.

use std::fs;

fn main_rs() -> String {
    fs::read_to_string("src/main.rs").expect("read main.rs")
}

#[test]
fn closing_hides_rather_than_exits_on_macos_only() {
    let main = main_rs();

    let cfg_at = main
        .find("#[cfg(target_os = \"macos\")]")
        .expect("a macos cfg block");
    let prevent_at = main
        .find("api.prevent_close()")
        .expect("close is prevented so the window can hide");
    let hide_at = main.find("window.hide()").expect("the window hides");

    assert!(
        cfg_at < prevent_at && prevent_at < hide_at,
        "hide-on-close is not confined to macOS"
    );
    assert!(
        main.contains("#[cfg(not(target_os = \"macos\"))]"),
        "the non-macOS close path is no longer explicit"
    );
}

#[test]
fn the_app_outlives_its_last_window_on_macos() {
    let main = main_rs();

    let exit_at = main
        .find("code: None")
        .expect("the no-code exit request is matched");
    let prevent_at = main
        .find("api.prevent_exit()")
        .expect("the exit is prevented");
    assert!(
        exit_at < prevent_at,
        "prevent_exit is not tied to the windowless exit request"
    );

    let macos_before_exit = main[..exit_at].rfind("#[cfg(target_os = \"macos\")]");
    assert!(
        macos_before_exit.is_some(),
        "the keep-alive arm is not confined to macOS"
    );
}

#[test]
fn the_dock_reopens_the_window() {
    let main = main_rs();

    assert!(
        main.contains("RunEvent::Reopen"),
        "no Reopen handling: the Dock icon would do nothing"
    );
    let reopen_at = main.find("RunEvent::Reopen").expect("reopen arm");
    let show_at = main[reopen_at..]
        .find("window.show()")
        .expect("reopen shows the hidden window");
    let rebuild_at = main[reopen_at..]
        .find("build_main_window(app)")
        .expect("reopen rebuilds a destroyed window");
    assert!(
        show_at < rebuild_at,
        "show is the primary path, rebuild the fallback"
    );
}

#[test]
fn geometry_is_saved_where_it_is_restored_from() {
    let main = main_rs();

    // Saved on close and on exit, restored in the builder — all three through
    // the same state_path, so the file cannot fork.
    assert_eq!(
        main.matches("remember_geometry(").count(),
        3,
        "expected the definition plus the close and exit call sites"
    );
    assert!(main.contains("window_state::load_from(&window_state::state_path())"));
    assert_eq!(
        main.matches("window_state::state_path()").count(),
        2,
        "save and restore should be the only state_path readers in the shell"
    );
}
