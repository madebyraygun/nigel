---
id: TASK-33.24
title: Silence the webview's context menu and the keystroke beep
status: To Do
assignee: []
created_date: '2026-08-20 23:48'
labels:
  - tauri
  - ui
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two smaller WKWebView tells. Right-clicking anywhere in the app raises the webview's own context menu — Reload and all — which no native app shows on its chrome. Suppress it outside editable fields; inputs, textareas and contenteditable keep the native text-editing menu (Cut, Copy, Paste, spelling, Look Up) untouched.

Second, macOS plays the alert sound for keydowns the webview handles without preventDefault. The register grid answers arrows, PageUp/PageDown, Home/End, Enter and Space; whether each handled key also prevents the default decides whether scrolling stays contained and the funk stays silent. Audit the handled-key paths in the desktop shell and pin what the audit finds.

Per docs/native-feel.md, host-conditional suppression is placed in crates/nigel-desktop or web/apps/app, never in @nigel/ui — though preventDefault on a key a component genuinely handled is correct everywhere and belongs to the component.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Right-click on app chrome shows no menu in the desktop shell; text fields keep the native editing menu
- [ ] #2 The browser build's context-menu behavior is unchanged
- [ ] #3 Keys the register grid handles produce no system alert sound in the desktop shell
- [ ] #4 Host-conditional suppression lives in the shell or web/apps/app, never in @nigel/ui
<!-- AC:END -->
