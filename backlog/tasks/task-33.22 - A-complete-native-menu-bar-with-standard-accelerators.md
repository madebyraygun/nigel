---
id: TASK-33.22
title: A complete native menu bar with standard accelerators
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-20 23:48'
updated_date: '2026-08-21 14:34'
labels:
  - tauri
  - ui
  - macos
milestone: m-0
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The desktop shell ships Tauri's default menu — nothing Nigel authored. A Mac app's behavioral identity lives in its menu bar: every standard chord a user reaches for (Cmd+W, Cmd+M, Cmd+1, Cmd+,) is answered there, and its absence is one of the last structural tells that this is a web app in a window.

The bar the shell should author: the app menu with a native About panel and the Settings item TASK-33.14 hangs its window from; File with Import Statement… (Cmd+O, routing into the existing pick_import_file flow), New Invoice (Cmd+N), Export…, and Close Window (Cmd+W); Edit built from the predefined Undo/Redo/Cut/Copy/Paste/Select All items — not optional, since WKWebView's clipboard chords stop working in text fields the moment a custom menu omits them; View with Cmd+1 through Cmd+9 jumping to sidebar screens, Toggle Sidebar, and the predefined Enter Full Screen; Window with the predefined Minimize and Zoom, marked as the NSApp windows menu so macOS manages the window list and tiling itself; Help.

Menu selections reach the SPA as events through the desktop api client — the same seam exports use — so no wc-* component learns it is running under Tauri, and accelerators are abstracted per platform rather than hardcoded to Cmd.

One deliberate reclamation: wc-register-table yields Cmd+F on the reasoning that find-in-page owns it, but WKWebView has no find-in-page UI, so in the desktop shell the chord currently does nothing. Edit > Find can carry Cmd+F to focus the register filter — through the menu, so the component keeps yielding modified keys and the browser build is untouched.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The shell authors a complete menu bar — app menu, File, Edit, View, Window, Help — with the standard items and accelerators
- [ ] #2 Clipboard, undo and select-all chords keep working in every text field, carried by the predefined Edit items
- [ ] #3 Cmd+1 through Cmd+9 navigate to sidebar screens, and Edit > Find focuses the register filter in the desktop shell
- [ ] #4 Menu selections reach the SPA as events through the api client; no @nigel/ui component detects the host
- [ ] #5 Accelerators are platform-abstracted, and the browser build's behavior is unchanged
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Rust: new crates/nigel-desktop/src/menu.rs building the bar via tauri::menu — macOS: app menu (predefined About with AboutMetadata, Settings item id settings accel CmdOrCtrl+Comma, predefined Services/Hide/HideOthers/ShowAll/Quit), File (Import Statement id import CmdOrCtrl+O, New Invoice id new-invoice CmdOrCtrl+N, Export id export CmdOrCtrl+E, predefined CloseWindow), Edit (predefined Undo/Redo/Cut/Copy/Paste/SelectAll — the clipboard-load-bearing set — plus Find id find CmdOrCtrl+F), View (all 13 nav screens in registry order as navigate:<id> items, accelerators CmdOrCtrl+1..9 on the first nine only, Toggle Sidebar id toggle-sidebar CmdOrCtrl+Alt+S, predefined Fullscreen cfg macos), Window (predefined Minimize/Maximize, set_as_windows_menu_for_nsapp on macOS), Help (set_as_help_menu_for_nsapp on macOS; no external links — the opener boundary belongs to TASK-33.13). Windows/Linux ship the same bar in-window with the platform shape: Settings and Quit in File, About in Help, no app menu.
2. main.rs wires .menu(menu::build) and .on_menu_event forwarding every custom id as one app event menu-command with payload {id}. No capability change: core:event:allow-listen already covers listening.
3. Seam: client.ts gains MenuSource — kind none (FetchApiClient) or kind native with onCommand(handler) returning unsubscribe (DesktopApiClient, subscribing to menu-command and mapping ids to a typed MenuCommand union: navigate guarded by isScreenId, find, import, new-invoice, export, settings, toggle-sidebar; unknown ids dropped). Matches the ExportTarget/ImportSource discriminated-union pattern; no wc-* component learns the host.
4. nigel-app subscribes when kind is native and translates: navigate:<screen> to navigate(screen); new-invoice to navigate(invoices, new=1) — the existing view; settings to navigate(settings), the recorded bridge TASK-33.14 later re-points to its own window on the Rust side without SPA change; export to navigate(reports) — the export surface; context-sensitive export flagged follow-up; toggle-sidebar flips sidebarCollapsed; find and import navigate then set a one-shot menuIntent signal on the app store which register.ts consumes by focusing the search box (already route-driven via q) and import.ts consumes by invoking its existing pick flow.
5. Drift guard in the crate's own idiom: a Rust test reads web/apps/app/src/screens/registry.ts source and asserts menu.rs's 13 (id, navLabel) pairs appear in the same order, so menu and sidebar cannot drift silently; plus unit tests over the id table and a source-text test that main.rs wires .menu( and .on_menu_event(.
6. Web tests: FetchApiClient answers none; DesktopApiClient maps and drops unknown ids; nigel-app with a fake native MenuSource navigates, flips the sidebar, and sets intents; register/import intent-consumption tests via the fake client.
7. Gates: cargo fmt --check and cargo test in crates/nigel-desktop, full web gate (typecheck, lint, test, build), check-no-real-data by exit status. Linux verification: run the shell, exercise the in-window GTK bar, Ctrl+1..9, find focus, import pick. PR body carries the macOS checklist: app-menu identity and About panel, clipboard chords in text fields, Cmd+Comma, windows-menu tiling, fullscreen item.
8. Branch feat/desktop-menubar in worktree /home/dalton/Dev/nigel/wt-menubar (own npm ci). Files: menu.rs new, main.rs, lib.rs, two new crate tests; web client.ts, desktop-client.ts, nigel-app.ts, app-store.ts, register.ts, import.ts, fake-api-client.ts and tests. Overlap with open PRs is confined to additive edits in client.ts and fake-api-client.ts (both touched by 38 and 40) — different regions, small rebase risk, accepted.
<!-- SECTION:PLAN:END -->
