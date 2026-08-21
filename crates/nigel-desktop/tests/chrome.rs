//! The launch-paint promises: the window starts hidden and shows painted,
//! and its own color is the theme's canvas rather than webview white. Read
//! off the source — CI has no display server — with the color pinned against
//! the theme tokens the SPA actually paints from.

use std::fs;

fn main_rs() -> String {
    fs::read_to_string("src/main.rs").expect("read main.rs")
}

#[test]
fn the_shell_and_the_theme_agree_on_the_canvas() {
    let chrome = fs::read_to_string("src/chrome.rs").expect("read chrome.rs");
    let theme = fs::read_to_string("../../web/packages/theme/src/tokens/color.ts")
        .expect("read the theme's color tokens");

    // Anchored to the constant on one side and the canvas token's
    // declaration on the other: a hex that merely appears somewhere in
    // either file — a comment, an unrelated token — cannot satisfy this.
    for (constant, hex) in [("BG_LIGHT", "#f3f2f7"), ("BG_DARK", "#17171d")] {
        assert!(
            chrome.contains(&format!("pub const {constant}: &str = \"{hex}\"")),
            "src/chrome.rs no longer declares {constant} as {hex}"
        );
        assert!(
            theme.contains(&format!("--wa-color-bg: {hex}")),
            "the theme's canvas token moved off {hex}; update chrome.rs to match"
        );
    }
}

#[test]
fn the_window_starts_hidden() {
    let main = main_rs();

    let visible_at = main
        .find(".visible(false)")
        .expect("the window is not built hidden");
    let build_at = main.find(".build()?").expect("build()");
    assert!(
        visible_at < build_at,
        "visible(false) is not applied to the window builder"
    );
}

#[test]
fn the_frontend_shows_the_window_with_a_timed_fallback() {
    let main = main_rs();

    assert!(
        main.contains("chrome::frontend_ready"),
        "the ready command is not registered: nothing would ever show the window"
    );
    assert!(
        main.contains("chrome::set_chrome_background"),
        "the background command is not registered: resize edges would stay on the boot guess"
    );

    // The fallback must be armed for the first build and again for a
    // Reopen rebuild — the rebuilt window starts hidden too, and a wedged
    // frontend there would otherwise hide it forever.
    let setup_at = main.find(".setup(").expect("a setup hook");
    let run_at = main.find(".run(").expect("the run loop");
    let calls: Vec<usize> = main
        .match_indices("spawn_show_fallback(")
        .map(|(at, _)| at)
        .filter(|at| {
            main[*at..].starts_with("spawn_show_fallback(app")
                || main[*at..].starts_with("spawn_show_fallback(handle")
        })
        .collect();
    assert!(
        calls.iter().any(|at| (setup_at..run_at).contains(at)),
        "setup no longer arms the show fallback"
    );
    assert!(
        calls.iter().any(|at| *at > run_at),
        "the Reopen rebuild no longer arms a show fallback"
    );
    let reopen_at = main.find("tauri::RunEvent::Reopen").expect("reopen arm");
    assert!(
        main[reopen_at..].contains("chrome::Shown>().reset()"),
        "a rebuilt window must count as never shown, or the fallback skips it"
    );
}

#[test]
fn the_fallback_only_reveals_a_window_nothing_else_showed() {
    let main = main_rs();
    let fallback_at = main
        .find("fn spawn_show_fallback")
        .expect("the show fallback exists");
    let fallback = &main[fallback_at..];

    assert!(
        fallback.contains("chrome::Shown>().first()"),
        "the fallback no longer consults ever-shown state: it would resurrect a window the user closed"
    );
    // An unreadable visibility must count as hidden — the fallback exists
    // to guarantee something shows.
    assert!(
        fallback.contains("is_visible().unwrap_or(false)"),
        "the fallback no longer treats an unreadable visibility as hidden"
    );
    assert!(
        !main.contains("is_visible().unwrap_or(true)"),
        "unwrap_or(true) inverts the fallback: a visibility error would skip the show"
    );
}
