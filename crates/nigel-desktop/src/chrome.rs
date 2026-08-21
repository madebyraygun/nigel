//! The window's own color and first appearance, so the shell never paints a
//! color the page would not.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::webview::Color;

/// Whether the main window has ever been shown, managed as tauri state.
///
/// The show fallback consults it so it only ever reveals a window nothing
/// else managed to show: without the distinction, a fallback firing after
/// the user closed (hid) the window would bring it back uninvited.
#[derive(Default)]
pub struct Shown(AtomicBool);

impl Shown {
    /// Record a show, answering whether it was the first.
    pub fn first(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }

    /// A rebuilt window starts unshown again.
    pub fn reset(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The theme's canvas in light mode, duplicated from
/// `web/packages/theme/src/tokens/color.ts`; `tests/chrome.rs` fails when
/// the two drift.
pub const BG_LIGHT: &str = "#f3f2f7";
/// The dark-mode canvas; see [`BG_LIGHT`].
pub const BG_DARK: &str = "#17171d";

/// The window color for an OS theme. The SPA refines this through
/// [`set_chrome_background`] once it has resolved any stored override.
pub fn background_for(theme: tauri::Theme) -> Color {
    let hex = match theme {
        tauri::Theme::Dark => BG_DARK,
        _ => BG_LIGHT,
    };
    let (r, g, b) = rgb(hex);
    Color(r, g, b, 255)
}

/// Show the window: the SPA settled its first update.
#[tauri::command]
pub fn frontend_ready(window: tauri::WebviewWindow, shown: tauri::State<Shown>) {
    shown.first();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Keep the window's own background on the SPA's resolved palette, so a
/// resize that outruns the webview shows theme background at the edges.
#[tauri::command]
pub fn set_chrome_background(window: tauri::WebviewWindow, mode: String) {
    let theme = match mode.as_str() {
        "dark" => tauri::Theme::Dark,
        "light" => tauri::Theme::Light,
        // An unknown mode is a frontend bug; keeping the current color is
        // the whole of the right response.
        _ => return,
    };
    let _ = window.set_background_color(Some(background_for(theme)));
}

/// `#rrggbb` to components. Only ever fed the constants above, so a
/// malformed literal is a programmer error caught by the unit tests.
fn rgb(hex: &str) -> (u8, u8, u8) {
    let parse = |range| u8::from_str_radix(&hex[range], 16).expect("hex canvas constant");
    (parse(1..3), parse(3..5), parse(5..7))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canvas_constants_parse() {
        assert_eq!(rgb(BG_LIGHT), (0xf3, 0xf2, 0xf7));
        assert_eq!(rgb(BG_DARK), (0x17, 0x17, 0x1d));
    }

    #[test]
    fn a_show_is_first_exactly_once_until_reset() {
        let shown = Shown::default();
        assert!(shown.first(), "a fresh window has never shown");
        assert!(!shown.first(), "a second show is not the first");
        shown.reset();
        assert!(shown.first(), "a rebuilt window starts unshown");
    }

    #[test]
    fn each_theme_gets_its_own_canvas_at_full_alpha() {
        assert_eq!(
            background_for(tauri::Theme::Light),
            Color(0xf3, 0xf2, 0xf7, 255)
        );
        assert_eq!(
            background_for(tauri::Theme::Dark),
            Color(0x17, 0x17, 0x1d, 255)
        );
    }
}
