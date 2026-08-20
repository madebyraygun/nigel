---
id: TASK-33.22
title: A complete native menu bar with standard accelerators
status: To Do
assignee: []
created_date: '2026-08-20 23:48'
labels:
  - tauri
  - ui
  - macos
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
