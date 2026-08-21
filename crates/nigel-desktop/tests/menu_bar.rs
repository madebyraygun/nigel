//! The menu bar mirrors the sidebar, and the shell actually installs it.
//!
//! Both read off source rather than a running app: building a window needs a
//! display server, which CI has not got, and the sidebar's order lives in the
//! SPA's screen registry — another language, same repository.

use std::fs;

use nigel_desktop::menu::NAV_SCREENS;

/// The registry the sidebar renders from, relative to this crate.
const REGISTRY: &str = "../../web/apps/app/src/screens/registry.ts";

/// The View menu lists what the sidebar lists, in the sidebar's order.
///
/// The menu's copy of the screens lives in `menu::NAV_SCREENS`; the sidebar's
/// lives in `registry.ts`. This test is the seam between the two languages:
/// adding, removing, renaming or reordering a nav screen in one place fails
/// here until the other follows.
#[test]
fn the_view_menu_mirrors_the_screen_registry() {
    let registry = fs::read_to_string(REGISTRY).expect("read the SPA screen registry");

    let mut previous = 0usize;
    for (screen, label) in NAV_SCREENS {
        let id_marker = format!("id: '{screen}'");
        let at = registry[previous..]
            .find(&id_marker)
            .map(|found| previous + found)
            .unwrap_or_else(|| {
                panic!("{REGISTRY} lacks {id_marker} after byte {previous} — the menu and the sidebar disagree on screens or their order")
            });

        let label_marker = format!("navLabel: '{label}'");
        let window = &registry[at..(at + 400).min(registry.len())];
        assert!(
            window.contains(&label_marker),
            "{REGISTRY} names '{screen}' differently than the menu's '{label}'"
        );
        previous = at;
    }

    let in_nav = registry.matches("inNav: true").count();
    assert_eq!(
        in_nav,
        NAV_SCREENS.len(),
        "{REGISTRY} has {in_nav} sidebar screens but the View menu lists {} — a screen joined or left the sidebar without the menu following",
        NAV_SCREENS.len()
    );
}

/// The builder actually installs the bar and forwards its selections.
#[test]
fn the_shell_installs_the_menu_and_forwards_selections() {
    let main = fs::read_to_string("src/main.rs").expect("read main.rs");

    assert!(
        main.contains(".menu(menu::build)"),
        "src/main.rs does not install the menu bar"
    );
    assert!(
        main.contains(".on_menu_event(menu::forward)"),
        "src/main.rs does not forward menu selections"
    );
}
