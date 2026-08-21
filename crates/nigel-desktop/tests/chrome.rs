//! The launch-paint promises: the window starts hidden and shows painted,
//! and its own color is the theme's canvas rather than webview white. Read
//! off the source — CI has no display server — with the color pinned against
//! the theme tokens the SPA actually paints from.

use std::fs;

#[test]
fn the_shell_and_the_theme_agree_on_the_canvas() {
    let chrome = fs::read_to_string("src/chrome.rs").expect("read chrome.rs");
    let theme = fs::read_to_string("../../web/packages/theme/src/tokens/color.ts")
        .expect("read the theme's color tokens");

    for hex in ["#f3f2f7", "#17171d"] {
        assert!(
            chrome.contains(hex),
            "src/chrome.rs no longer declares {hex}"
        );
        assert!(
            theme.contains(hex),
            "the theme moved off {hex}; update chrome.rs to match"
        );
    }
}

#[test]
fn the_window_starts_hidden() {
    let main = fs::read_to_string("src/main.rs").expect("read main.rs");

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
    let main = fs::read_to_string("src/main.rs").expect("read main.rs");

    assert!(
        main.contains("chrome::frontend_ready"),
        "the ready command is not registered: nothing would ever show the window"
    );
    assert!(
        main.contains("from_secs(4)") && main.contains("is_visible"),
        "no fallback: a wedged frontend would leave an invisible process"
    );
    assert!(
        main.contains("chrome::set_chrome_background"),
        "the background command is not registered: resize edges would stay on the boot guess"
    );
}
