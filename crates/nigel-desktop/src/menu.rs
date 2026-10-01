//! The application menu bar: structure, accelerators, and the event bridge.
//!
//! Selections the platform cannot answer natively are forwarded to the SPA as
//! one event, [`MENU_EVENT`], whose payload is the command id. The SPA maps
//! ids to actions behind its api-client seam, so the menu can grow without a
//! capability change and an id the SPA does not know is simply dropped.

use tauri::menu::{AboutMetadata, Menu, MenuEvent, MenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Runtime};

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
///
/// `web/apps/app/src/api/client.ts` mirrors this list as `MENU_COMMAND_IDS`;
/// `tests/menu_bar.rs` fails the build when the two drift.
pub const COMMANDS: [&str; 4] = ["import", "new-invoice", "find", "toggle-sidebar"];

/// The ids of the items the shell answers itself on Linux, where GTK has no
/// predefined Quit or Close Window. They never reach the SPA.
pub const QUIT: &str = "quit";
pub const CLOSE_WINDOW: &str = "close-window";

/// What the shell does with a selection it answers itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellAction {
    Quit,
    CloseWindow,
}

/// The shell's own action for a menu id, if the id is [`QUIT`] or
/// [`CLOSE_WINDOW`].
pub fn shell_action(menu_id: &str) -> Option<ShellAction> {
    match menu_id {
        QUIT => Some(ShellAction::Quit),
        CLOSE_WINDOW => Some(ShellAction::CloseWindow),
        _ => None,
    }
}

/// The command id a menu selection carries to the SPA, if it is ours.
///
/// Predefined items (clipboard, window management, quit) are handled by the
/// platform and never reach the SPA; their ids answer `None` here, as do the
/// shell's own [`QUIT`] and [`CLOSE_WINDOW`].
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

/// Answer a selection: the shell's own items here, everything else forwarded
/// to the SPA. The window subscribes through the api-client seam; nothing
/// else listens.
pub fn forward<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id().as_ref();
    match shell_action(id) {
        Some(ShellAction::Quit) => app.exit(0),
        // Through the window's close path, so the geometry save and the
        // last-window exit apply exactly as they do to the title-bar button.
        Some(ShellAction::CloseWindow) => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.close();
            }
        }
        None => {
            if let Some(id) = command_id(id) {
                // A window that is gone has nobody to tell; dropping the
                // selection is the whole of what can be done with it.
                let _ = app.emit(MENU_EVENT, id.to_owned());
            }
        }
    }
}

/// The platform a bar is laid out for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    /// GTK, which every other desktop target builds against.
    Linux,
}

impl Platform {
    /// The platform this binary was built for.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// An item the platform implements, built from muda's predefined items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Quit,
    CloseWindow,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Fullscreen,
    Minimize,
    Maximize,
}

impl Native {
    /// Whether muda renders this item on GTK. Its GTK backend implements only
    /// these (and separators) and silently skips every other predefined
    /// item, leaving no label and the separators around it doubled.
    pub const fn on_gtk(self) -> bool {
        matches!(
            self,
            Native::About | Native::Cut | Native::Copy | Native::Paste | Native::SelectAll
        )
    }
}

/// One row of a submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// An item of ours: forwarded to the SPA, or answered by the shell.
    Command {
        id: String,
        label: &'static str,
        accelerator: Option<String>,
    },
    Native(Native),
    Separator,
}

/// One top-level submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submenu {
    /// A tauri well-known id, for a submenu AppKit should manage.
    pub id: Option<&'static str>,
    pub label: &'static str,
    pub entries: Vec<Entry>,
}

/// The accelerator for Toggle Sidebar.
///
/// macOS takes the HIG's Show/Hide Sidebar chord. Elsewhere no Ctrl+Alt
/// chord is safe: Windows reads AltGr as Ctrl+Alt, so one would swallow a
/// character some layouts type with AltGr. Ctrl+Shift+B is layout-independent
/// and bound to nothing in a text field.
pub const fn toggle_sidebar_accelerator(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "Ctrl+Cmd+S",
        Platform::Windows | Platform::Linux => "Ctrl+Shift+B",
    }
}

/// The bar for a platform.
///
/// macOS hangs Settings and Quit off the app menu and marks the Window and
/// Help submenus for AppKit's own management. Windows renders the bar
/// in-window with Settings and Quit in File and About in Help. Linux has the
/// Windows shape with Quit and Close Window as the shell's own items, and
/// without Undo/Redo or a Window submenu: GTK has none of those predefined
/// items, and the webview's own undo chords work without a menu entry.
pub fn layout(platform: Platform) -> Vec<Submenu> {
    let mac = platform == Platform::MacOs;
    let linux = platform == Platform::Linux;
    let mut bar = Vec::new();

    if mac {
        bar.push(Submenu {
            id: None,
            label: "Nigel",
            entries: vec![
                Entry::Native(Native::About),
                Entry::Separator,
                settings_entry(),
                Entry::Separator,
                Entry::Native(Native::Services),
                Entry::Separator,
                Entry::Native(Native::Hide),
                Entry::Native(Native::HideOthers),
                Entry::Native(Native::ShowAll),
                Entry::Separator,
                Entry::Native(Native::Quit),
            ],
        });
    }

    let mut file = vec![
        command("import", "Import Statement…", "CmdOrCtrl+O"),
        command("new-invoice", "New Invoice", "CmdOrCtrl+N"),
        Entry::Separator,
        if linux {
            command(CLOSE_WINDOW, "Close Window", "CmdOrCtrl+W")
        } else {
            Entry::Native(Native::CloseWindow)
        },
    ];
    if !mac {
        file.extend([
            Entry::Separator,
            settings_entry(),
            Entry::Separator,
            if linux {
                command(QUIT, "Quit", "CmdOrCtrl+Q")
            } else {
                Entry::Native(Native::Quit)
            },
        ]);
    }
    bar.push(Submenu {
        id: None,
        label: "File",
        entries: file,
    });

    let mut edit = Vec::new();
    if !linux {
        edit.extend([
            Entry::Native(Native::Undo),
            Entry::Native(Native::Redo),
            Entry::Separator,
        ]);
    }
    edit.extend([
        Entry::Native(Native::Cut),
        Entry::Native(Native::Copy),
        Entry::Native(Native::Paste),
        Entry::Native(Native::SelectAll),
        Entry::Separator,
        command("find", "Find", "CmdOrCtrl+F"),
    ]);
    bar.push(Submenu {
        id: None,
        label: "Edit",
        entries: edit,
    });

    let mut view: Vec<Entry> = NAV_SCREENS
        .iter()
        .enumerate()
        .map(|(index, (screen, label))| Entry::Command {
            id: format!("navigate:{screen}"),
            label,
            accelerator: (index < ACCELERATED).then(|| format!("CmdOrCtrl+{}", index + 1)),
        })
        .collect();
    view.extend([
        Entry::Separator,
        command(
            "toggle-sidebar",
            "Toggle Sidebar",
            toggle_sidebar_accelerator(platform),
        ),
    ]);
    if mac {
        view.extend([Entry::Separator, Entry::Native(Native::Fullscreen)]);
    }
    bar.push(Submenu {
        id: None,
        label: "View",
        entries: view,
    });

    // The well-known ids matter: tauri hands submenus carrying them to AppKit
    // (`set_as_windows_menu_for_nsapp` / `set_as_help_menu_for_nsapp`) after it
    // installs the bar into NSApp. Calling those methods here would be too
    // early — NSApp has no main menu yet, so muda cannot find the NSMenu and
    // returns without doing anything.
    if !linux {
        bar.push(Submenu {
            id: Some(tauri::menu::WINDOW_SUBMENU_ID),
            label: "Window",
            entries: vec![
                Entry::Native(Native::Minimize),
                Entry::Native(Native::Maximize),
            ],
        });
    }

    bar.push(Submenu {
        id: Some(tauri::menu::HELP_SUBMENU_ID),
        label: "Help",
        entries: if mac {
            Vec::new()
        } else {
            vec![Entry::Native(Native::About)]
        },
    });

    bar
}

/// Build the bar for the platform this binary runs on.
pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    for submenu in layout(Platform::current()) {
        menu.append(&render(app, &submenu)?)?;
    }
    Ok(menu)
}

fn render<R: Runtime>(
    app: &AppHandle<R>,
    submenu: &Submenu,
) -> tauri::Result<tauri::menu::Submenu<R>> {
    let mut builder = match submenu.id {
        Some(id) => SubmenuBuilder::with_id(app, id, submenu.label),
        None => SubmenuBuilder::new(app, submenu.label),
    };
    for entry in &submenu.entries {
        builder = match entry {
            Entry::Separator => builder.separator(),
            Entry::Command {
                id,
                label,
                accelerator,
            } => builder.item(&MenuItem::with_id(
                app,
                id,
                *label,
                true,
                accelerator.as_deref(),
            )?),
            Entry::Native(native) => match native {
                Native::About => builder.about(Some(about_metadata())),
                Native::Services => builder.services(),
                Native::Hide => builder.hide(),
                Native::HideOthers => builder.hide_others(),
                Native::ShowAll => builder.show_all(),
                Native::Quit => builder.quit(),
                Native::CloseWindow => builder.close_window(),
                Native::Undo => builder.undo(),
                Native::Redo => builder.redo(),
                Native::Cut => builder.cut(),
                Native::Copy => builder.copy(),
                Native::Paste => builder.paste(),
                Native::SelectAll => builder.select_all(),
                Native::Fullscreen => builder.fullscreen(),
                Native::Minimize => builder.minimize(),
                Native::Maximize => builder.maximize(),
            },
        };
    }
    builder.build()
}

fn settings_entry() -> Entry {
    // The navigation id, not a command of its own: opening Settings is exactly
    // what a View-menu selection of it does, so it rides the same path.
    command("navigate:settings", "Settings…", "CmdOrCtrl+,")
}

fn command(id: &str, label: &'static str, accelerator: &str) -> Entry {
    Entry::Command {
        id: id.to_owned(),
        label,
        accelerator: Some(accelerator.to_owned()),
    }
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

    const PLATFORMS: [Platform; 3] = [Platform::MacOs, Platform::Windows, Platform::Linux];

    fn submenu(platform: Platform, label: &str) -> Option<Submenu> {
        layout(platform)
            .into_iter()
            .find(|menu| menu.label == label)
    }

    fn commands(platform: Platform) -> Vec<(String, Option<String>)> {
        layout(platform)
            .into_iter()
            .flat_map(|menu| menu.entries)
            .filter_map(|entry| match entry {
                Entry::Command {
                    id, accelerator, ..
                } => Some((id, accelerator)),
                _ => None,
            })
            .collect()
    }

    fn accelerator_of(platform: Platform, id: &str) -> Option<String> {
        commands(platform)
            .into_iter()
            .find(|(command, _)| command == id)
            .and_then(|(_, accelerator)| accelerator)
    }

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
    fn the_shells_own_items_are_answered_not_forwarded() {
        assert_eq!(shell_action(QUIT), Some(ShellAction::Quit));
        assert_eq!(shell_action(CLOSE_WINDOW), Some(ShellAction::CloseWindow));
        assert_eq!(command_id(QUIT), None);
        assert_eq!(command_id(CLOSE_WINDOW), None);
        assert_eq!(shell_action("import"), None);
    }

    #[test]
    fn nine_screens_carry_number_accelerators() {
        assert!(ACCELERATED <= NAV_SCREENS.len());
        assert_eq!(ACCELERATED, 9);
    }

    #[test]
    fn every_item_of_ours_does_something() {
        for platform in PLATFORMS {
            for (id, _) in commands(platform) {
                assert!(
                    command_id(&id).is_some() || shell_action(&id).is_some(),
                    "{platform:?}: '{id}' is neither forwarded nor answered by the shell"
                );
            }
        }
    }

    #[test]
    fn separators_only_ever_sit_between_items() {
        for platform in PLATFORMS {
            for menu in layout(platform) {
                let rendered: Vec<&Entry> = menu
                    .entries
                    .iter()
                    .filter(|entry| match entry {
                        Entry::Native(native) => platform != Platform::Linux || native.on_gtk(),
                        _ => true,
                    })
                    .collect();
                let is_separator = |entry: &&Entry| matches!(entry, Entry::Separator);
                assert!(
                    !rendered.first().is_some_and(is_separator)
                        && !rendered.last().is_some_and(is_separator),
                    "{platform:?} {}: a separator at an edge",
                    menu.label
                );
                assert!(
                    !rendered
                        .windows(2)
                        .any(|pair| is_separator(&pair[0]) && is_separator(&pair[1])),
                    "{platform:?} {}: two separators in a row",
                    menu.label
                );
            }
        }
    }

    #[test]
    fn linux_uses_only_what_gtk_renders() {
        for menu in layout(Platform::Linux) {
            for entry in &menu.entries {
                if let Entry::Native(native) = entry {
                    assert!(
                        native.on_gtk(),
                        "{}: muda skips {native:?} on GTK",
                        menu.label
                    );
                }
            }
            assert!(
                !menu.entries.is_empty(),
                "{}: an empty submenu on Linux",
                menu.label
            );
        }
    }

    #[test]
    fn linux_quits_and_closes_through_the_shell() {
        let file = submenu(Platform::Linux, "File").expect("a File menu");
        let ids: Vec<&str> = file
            .entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Command { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids.last(), Some(&QUIT));
        assert!(ids.contains(&CLOSE_WINDOW));
        assert_eq!(
            accelerator_of(Platform::Linux, QUIT).as_deref(),
            Some("CmdOrCtrl+Q")
        );
        assert_eq!(
            accelerator_of(Platform::Linux, CLOSE_WINDOW).as_deref(),
            Some("CmdOrCtrl+W")
        );
        assert!(submenu(Platform::Linux, "Window").is_none());
    }

    #[test]
    fn macos_and_windows_keep_the_native_items() {
        for platform in [Platform::MacOs, Platform::Windows] {
            let natives: Vec<Native> = layout(platform)
                .into_iter()
                .flat_map(|menu| menu.entries)
                .filter_map(|entry| match entry {
                    Entry::Native(native) => Some(native),
                    _ => None,
                })
                .collect();
            for expected in [
                Native::Quit,
                Native::CloseWindow,
                Native::Undo,
                Native::Minimize,
            ] {
                assert!(natives.contains(&expected), "{platform:?}: no {expected:?}");
            }
            let window = submenu(platform, "Window").expect("a Window menu");
            assert_eq!(window.id, Some(tauri::menu::WINDOW_SUBMENU_ID));
            assert!(accelerator_of(platform, QUIT).is_none());
            assert!(accelerator_of(platform, CLOSE_WINDOW).is_none());
        }
    }

    #[test]
    fn toggle_sidebar_avoids_altgr() {
        assert_eq!(
            accelerator_of(Platform::MacOs, "toggle-sidebar").as_deref(),
            Some("Ctrl+Cmd+S")
        );
        for platform in [Platform::Windows, Platform::Linux] {
            for (id, accelerator) in commands(platform) {
                let Some(accelerator) = accelerator else {
                    continue;
                };
                let keys: Vec<String> = accelerator
                    .split('+')
                    .map(str::to_ascii_lowercase)
                    .collect();
                let ctrl = keys
                    .iter()
                    .any(|key| matches!(key.as_str(), "ctrl" | "control" | "cmdorctrl"));
                let alt = keys
                    .iter()
                    .any(|key| matches!(key.as_str(), "alt" | "option"));
                assert!(
                    !(ctrl && alt),
                    "{platform:?}: '{id}' is {accelerator}, which AltGr types through"
                );
            }
        }
    }
}
