//! The application menu bar: structure, accelerators, and the event bridge.
//!
//! Selections the platform cannot answer natively are forwarded to the SPA as
//! one event, [`MENU_EVENT`], whose payload is the command id. The SPA maps
//! ids to actions behind its api-client seam, so the menu can grow without a
//! capability change and an id the SPA does not know is simply dropped.

use tauri::menu::{AboutMetadata, Menu, MenuEvent, MenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Runtime};

/// The event a custom menu selection reaches the SPA through; the payload is
/// the command id as a plain string.
pub const MENU_EVENT: &str = "menu-command";

/// The navigable screens, in the sidebar's order.
///
/// This mirrors `web/apps/app/src/screens/registry.ts` — the menu shows what
/// the sidebar shows, in the order it shows it. `tests/menu_bar.rs`
/// fails the build when the two drift. The first [`ACCELERATED`] entries carry
/// `CmdOrCtrl+1..9`.
pub const NAV_SCREENS: [(&str, &str); 13] = [
    ("dashboard", "Dashboard"),
    ("register", "Register"),
    ("review", "Review"),
    ("import", "Import"),
    ("reports", "Reports"),
    ("accounts", "Accounts"),
    ("categories", "Categories"),
    ("rules", "Rules"),
    ("clients", "Clients"),
    ("invoices", "Invoices"),
    ("reconcile", "Reconcile"),
    ("undo", "Undo"),
    ("settings", "Settings"),
];

/// How many of [`NAV_SCREENS`] get a number accelerator.
pub const ACCELERATED: usize = 9;

/// The non-navigation command ids the menu emits.
pub const COMMANDS: [&str; 5] = [
    "settings",
    "import",
    "new-invoice",
    "find",
    "toggle-sidebar",
];

/// The command id a menu selection carries to the SPA, if it is ours.
///
/// Predefined items (clipboard, window management, quit) are handled by the
/// platform and never reach the SPA; their ids answer `None` here.
pub fn command_id(menu_id: &str) -> Option<&str> {
    if COMMANDS.contains(&menu_id) {
        return Some(menu_id);
    }
    let screen = menu_id.strip_prefix("navigate:")?;
    NAV_SCREENS
        .iter()
        .any(|(id, _)| *id == screen)
        .then_some(menu_id)
}

/// Forward a selection to the SPA. The window subscribes through the
/// api-client seam; nothing else listens.
pub fn forward<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if let Some(id) = command_id(event.id().as_ref()) {
        // A window that is gone has nobody to tell; dropping the selection is
        // the whole of what can be done with it.
        let _ = app.emit(MENU_EVENT, id.to_owned());
    }
}

/// Build the bar.
///
/// One structure, two platform shapes: macOS hangs Settings and Quit off the
/// app menu and marks the Window and Help submenus for AppKit's own
/// management; Windows and Linux render the same bar in-window, so Settings
/// and Quit live in File and About lives in Help.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;

    #[cfg(target_os = "macos")]
    {
        let app_menu = SubmenuBuilder::new(app, "Nigel")
            .about(Some(about_metadata()))
            .separator()
            .item(&settings_item(app)?)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;
        menu.append(&app_menu)?;
    }

    let file = SubmenuBuilder::new(app, "File")
        .item(&item(app, "import", "Import Statement…", "CmdOrCtrl+O")?)
        .item(&item(app, "new-invoice", "New Invoice", "CmdOrCtrl+N")?)
        .separator()
        .close_window();
    #[cfg(not(target_os = "macos"))]
    let file = file
        .separator()
        .item(&settings_item(app)?)
        .separator()
        .quit();
    menu.append(&file.build()?)?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator()
        .item(&item(app, "find", "Find", "CmdOrCtrl+F")?)
        .build()?;
    menu.append(&edit)?;

    let mut view = SubmenuBuilder::new(app, "View");
    for (index, (screen, label)) in NAV_SCREENS.iter().enumerate() {
        let accelerator = (index < ACCELERATED).then(|| format!("CmdOrCtrl+{}", index + 1));
        view = view.item(&MenuItem::with_id(
            app,
            format!("navigate:{screen}"),
            *label,
            true,
            accelerator.as_deref(),
        )?);
    }
    let view = view.separator().item(&item(
        app,
        "toggle-sidebar",
        "Toggle Sidebar",
        "CmdOrCtrl+Alt+S",
    )?);
    #[cfg(target_os = "macos")]
    let view = view.separator().fullscreen();
    menu.append(&view.build()?)?;

    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .build()?;
    #[cfg(target_os = "macos")]
    window.set_as_windows_menu_for_nsapp()?;
    menu.append(&window)?;

    let help = SubmenuBuilder::new(app, "Help");
    #[cfg(not(target_os = "macos"))]
    let help = help.about(Some(about_metadata()));
    let help = help.build()?;
    #[cfg(target_os = "macos")]
    help.set_as_help_menu_for_nsapp()?;
    menu.append(&help)?;

    Ok(menu)
}

fn settings_item<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<MenuItem<R>> {
    item(app, "settings", "Settings…", "CmdOrCtrl+,")
}

fn item<R: Runtime>(
    app: &AppHandle<R>,
    id: &str,
    label: &str,
    accelerator: &str,
) -> tauri::Result<MenuItem<R>> {
    MenuItem::with_id(app, id, label, true, Some(accelerator))
}

fn about_metadata() -> AboutMetadata<'static> {
    AboutMetadata {
        name: Some("Nigel".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_ids_are_forwarded() {
        assert_eq!(command_id("navigate:register"), Some("navigate:register"));
        assert_eq!(command_id("navigate:settings"), Some("navigate:settings"));
    }

    #[test]
    fn commands_are_forwarded() {
        for command in COMMANDS {
            assert_eq!(command_id(command), Some(command));
        }
    }

    #[test]
    fn foreign_and_predefined_ids_are_dropped() {
        assert_eq!(command_id("navigate:nowhere"), None);
        assert_eq!(command_id("copy"), None);
        assert_eq!(command_id(""), None);
    }

    #[test]
    fn nine_screens_carry_number_accelerators() {
        assert!(ACCELERATED <= NAV_SCREENS.len());
        assert_eq!(ACCELERATED, 9);
    }
}
