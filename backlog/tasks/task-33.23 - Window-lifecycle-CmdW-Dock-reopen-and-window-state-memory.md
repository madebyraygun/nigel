---
id: TASK-33.23
title: 'Window lifecycle: Cmd+W, Dock reopen, and window-state memory'
status: To Do
assignee: []
created_date: '2026-08-20 23:48'
labels:
  - tauri
  - macos
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Closing Nigel's window kills the process. On macOS an app survives its last window: Cmd+W closes, the Dock icon brings the window back, and it comes back where and how the user left it. The shell does none of this today, and after the menu bar it is the clearest behavioral wrapper tell left.

Three pieces, all shell-owned. Keep the app running when the last window closes on macOS, recreating or re-showing the main window from the run loop's Reopen event when the Dock icon is clicked. Persist window size and position across launches and restore them at build time, clamped to a visible screen and the existing 900x700 minimum. Leave Windows and Linux with their own conventions, where closing the window exits.

tauri-plugin-window-state would do the third piece but carries open macOS bugs — windows restoring at half their minimum size, hangs with undecorated windows — and the shell's plugin discipline is deliberate: one plugin today. A hand-rolled save-on-close/restore-on-build stays small and keeps that posture.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 On macOS, closing the window leaves Nigel running, and clicking the Dock icon brings the main window back
- [ ] #2 Window size and position persist across launches, restored clamped to a visible screen; first launch keeps the 1200x820 default
- [ ] #3 The 900x700 minimum still holds after restore
- [ ] #4 Windows and Linux are unchanged: closing the window exits
<!-- AC:END -->
