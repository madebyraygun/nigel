---
id: TASK-33.14
title: Open settings as its own window from a native menu
status: To Do
assignee: []
created_date: '2026-08-18 01:33'
updated_date: '2026-08-20 23:50'
labels:
  - tauri
  - ui
dependencies:
  - TASK-33.22
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On macOS a preferences window is its own window, opened with Cmd+, — not another page in the app's own navigation. Nigel's settings are a route like any other, which is one of the clearest remaining tells that the desktop app is a web app in a window.

This is shell-owned work: a native menu with a Preferences item, its accelerator abstracted per platform (Cmd on macOS, Ctrl elsewhere), and a second webview window pointed at the settings route. Boxcraft's native-feel conventions name exactly this boundary — menus, accelerators and window management belong to the shell, not to @nigel/ui, and no wc-* component should learn that it is running under Tauri.

The menu bar itself is TASK-33.22's work — the full bar with the standard items; this task hangs the Preferences item and its window off it. A macOS settings window has its own conventions worth keeping: fixed size or content-sized, minimize and zoom disabled, closed with Cmd+W.

The browser build keeps settings as a route, since a browser has no menu bar to hang it from.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A native application menu carries a Preferences item that opens settings in its own window
- [ ] #2 The accelerator is Cmd+, on macOS and the platform equivalent elsewhere, never hardcoded to one modifier
- [ ] #3 Closing the settings window leaves the main window as it was, and reopening reuses the existing window rather than stacking another
- [ ] #4 The browser build still reaches settings as a route, and no @nigel/ui component branches on the host
<!-- AC:END -->
