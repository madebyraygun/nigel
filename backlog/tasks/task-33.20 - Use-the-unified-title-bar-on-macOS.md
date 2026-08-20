---
id: TASK-33.20
title: Use the unified title bar on macOS
status: To Do
assignee: []
created_date: '2026-08-20 21:47'
updated_date: '2026-08-20 23:50'
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
Nigel's macOS desktop window currently keeps the standard native title bar visually separate from the app chrome. Configure the Tauri window to use macOS's unified title-bar treatment so the window reads as one native surface, while preserving the standard traffic-light controls and a reliable region for moving the window. This is shell-owned presentation: the shared SPA and @nigel/ui should not branch on macOS, and Windows/Linux window behavior should remain unchanged.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The packaged macOS desktop app uses the platform's unified title-bar treatment, with app chrome and window chrome reading as one surface
- [ ] #2 The standard macOS close, minimize, and zoom controls remain visible, correctly positioned, and operable
- [ ] #3 The unified chrome retains a reliable drag region without making interactive controls drag the window
- [ ] #4 Windows, Linux, and browser presentation are unchanged, and no @nigel/ui component detects the host platform
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. TitleBarStyle::Overlay with hidden_title(true) in the shell's window builder; traffic lights vertically centered in the 48px header via traffic_light_position (now in Tauri core).
2. Drag region through the seam, not markup: data-tauri-drag-region is unreliable through Lit shadow roots (the injected listener sees retargeted events), so add a beginWindowDrag() capability to the api client — the desktop client calls startDragging(), the browser client no-ops — wired to header mousedown at the app layer. Interactive controls in the header must not start a drag.
3. Handle title-bar double-click per the system zoom/minimize preference, since startDragging alone does not provide it.
4. Traffic-light clearance via a CSS custom property (e.g. --nc-titlebar-inset) set by the shell/app and consumed by wc-app-shell, so no component detects the platform.
5. Window background color matching the theme bg keeps the unified surface from flashing white mid-resize (shared concern with TASK-33.25).
<!-- SECTION:PLAN:END -->
