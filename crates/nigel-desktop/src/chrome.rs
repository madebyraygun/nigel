//! The window's own color and first appearance, so the shell never paints a
//! color the page would not.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::webview::Color;

/// Whether the main window has ever been shown, managed as tauri state.
///
/// Every show records itself here, and the two deferred ones — the ready
/// signal and the fallback timer — only reveal a window nothing else has
/// shown: without the distinction, whichever fired second after the user
/// closed (hid) the window would bring it back uninvited.
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

/// Show the window: the SPA settled its first update. Only a window nothing
/// has shown yet — a ready that arrives after the fallback showed the window
/// and the user closed it must not bring it back.
#[tauri::command]
pub fn frontend_ready(window: tauri::WebviewWindow, shown: tauri::State<Shown>) {
    if shown.first() {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// What the window takes from the SPA's palette: its own color, and its
/// appearance — pinned to an explicit in-app choice, or `None` to follow
/// the OS.
///
/// The appearance is what reaches the resize edges on macOS: WKWebView's
/// default background follows the window's appearance, and the window color
/// does not reach the webview layer there. Pinned to the in-app theme, the
/// two agree even when the OS is set the other way.
#[derive(Debug, PartialEq, Eq)]
pub struct Palette {
    pub mode: tauri::Theme,
    pub appearance: Option<tauri::Theme>,
}

/// A palette report from the SPA: `mode` is the resolved `light` or
/// `dark`, `source` is `explicit` for an in-app choice or `system` for one
/// that follows the OS. Anything else is a frontend bug and answers `None`.
pub fn palette_for(mode: &str, source: &str) -> Option<Palette> {
    let mode = match mode {
        "dark" => tauri::Theme::Dark,
        "light" => tauri::Theme::Light,
        _ => return None,
    };
    let appearance = match source {
        "explicit" => Some(mode),
        "system" => None,
        _ => return None,
    };
    Some(Palette { mode, appearance })
}

/// Keep the window on the SPA's resolved palette, so a resize that outruns
/// the webview shows theme background at the edges.
#[tauri::command]
pub fn set_chrome_background(window: tauri::WebviewWindow, mode: String, source: String) {
    // An unknown report is a frontend bug; keeping the current chrome is the
    // whole of the right response.
    let Some(palette) = palette_for(&mode, &source) else {
        return;
    };
    let _ = window.set_theme(palette.appearance);
    let _ = window.set_background_color(Some(background_for(palette.mode)));
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
    fn an_explicit_choice_pins_the_appearance_and_system_releases_it() {
        assert_eq!(
            palette_for("dark", "explicit"),
            Some(Palette {
                mode: tauri::Theme::Dark,
                appearance: Some(tauri::Theme::Dark),
            })
        );
        assert_eq!(
            palette_for("light", "explicit"),
            Some(Palette {
                mode: tauri::Theme::Light,
                appearance: Some(tauri::Theme::Light),
            })
        );
        assert_eq!(
            palette_for("dark", "system"),
            Some(Palette {
                mode: tauri::Theme::Dark,
                appearance: None,
            })
        );
    }

    #[test]
    fn a_malformed_palette_report_is_refused() {
        for (mode, source) in [
            ("Dark", "explicit"),
            ("system", "system"),
            ("", "explicit"),
            ("dark", "Explicit"),
            ("dark", ""),
            ("light", "override"),
        ] {
            assert_eq!(palette_for(mode, source), None, "{mode:?}/{source:?}");
        }
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
