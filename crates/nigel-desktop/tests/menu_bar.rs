//! The menu bar mirrors the sidebar, and the shell actually installs it.
//!
//! Everything here reads off source rather than a running app: building a
//! window needs a display server, which CI has not got, and the SPA's half of
//! each seam lives in another language in the same repository.

use std::fs;

use nigel_desktop::menu::{COMMANDS, NAV_SCREENS};

/// The registry the sidebar renders from, relative to this crate.
const REGISTRY: &str = "../../web/apps/app/src/screens/registry.ts";

/// Where the SPA declares the non-navigation command ids.
const CLIENT: &str = "../../web/apps/app/src/api/client.ts";

/// One screen as `registry.ts` declares it.
struct Entry<'a> {
    id: &'a str,
    nav_label: Option<&'a str>,
    in_nav: bool,
}

/// Every screen entry, in declaration order.
///
/// A text scan, not a parser: each entry is delimited by its `id: '…'` line,
/// and the fields of interest are string-searched within the delimited block.
/// All markers are ASCII, so the block boundaries are always char boundaries.
fn entries(registry: &str) -> Vec<Entry<'_>> {
    let marker = "id: '";
    let mut starts: Vec<usize> = Vec::new();
    let mut from = 0;
    while let Some(found) = registry[from..].find(marker) {
        starts.push(from + found);
        from += found + marker.len();
    }

    starts
        .iter()
        .enumerate()
        .map(|(index, &start)| {
            let end = starts.get(index + 1).copied().unwrap_or(registry.len());
            let block = &registry[start..end];
            let id = quoted(block, marker).expect("an id marker always opens a quote");
            Entry {
                id,
                nav_label: quoted(block, "navLabel: '"),
                in_nav: block.contains("inNav: true"),
            }
        })
        .collect()
}

/// The single-quoted string a marker opens, if the marker is present.
fn quoted<'a>(block: &'a str, marker: &str) -> Option<&'a str> {
    let after = block.find(marker)? + marker.len();
    let close = block[after..].find('\'')?;
    Some(&block[after..after + close])
}

/// The View menu lists what the sidebar lists, in the sidebar's order.
///
/// The menu's copy of the screens lives in `menu::NAV_SCREENS`; the sidebar's
/// lives in `registry.ts`. This test is the seam between the two languages:
/// adding, removing, renaming, reordering, or re-flagging a nav screen in one
/// place fails here until the other follows.
#[test]
fn the_view_menu_mirrors_the_screen_registry() {
    let raw = fs::read_to_string(REGISTRY).expect("read the SPA screen registry");
    // A commented-out entry still carries its markers; dropping `//` lines
    // keeps a screen "removed" that way from passing as present.
    let registry = raw
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let entries = entries(&registry);
    assert!(
        !entries.is_empty(),
        "{REGISTRY} has no `id: '…'` entries — the registry moved or changed shape"
    );

    let sidebar: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.in_nav)
        .map(|entry| entry.id)
        .collect();
    let menu: Vec<&str> = NAV_SCREENS.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        sidebar, menu,
        "{REGISTRY} and the View menu disagree on the sidebar screens or their order"
    );

    for (id, label) in NAV_SCREENS {
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .expect("membership was just asserted");
        assert_eq!(
            entry.nav_label,
            Some(label),
            "{REGISTRY} names '{id}' differently than the menu"
        );
    }
}

/// The command ids match the SPA's `MENU_COMMAND_IDS` exactly.
///
/// Same seam, other half: a command renamed on either side would leave every
/// same-language test green while the menu item goes silently inert.
#[test]
fn the_menu_commands_mirror_the_client_union() {
    let client = fs::read_to_string(CLIENT).expect("read the SPA api client");
    let declaration = client
        .split("MENU_COMMAND_IDS = [")
        .nth(1)
        .and_then(|after| after.split(']').next())
        .expect("client.ts declares MENU_COMMAND_IDS");

    let spa: Vec<&str> = declaration.split('\'').skip(1).step_by(2).collect();
    assert_eq!(
        spa, COMMANDS,
        "{CLIENT} MENU_COMMAND_IDS and menu::COMMANDS disagree — a menu item was added or renamed on one side only"
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
