---
id: TASK-33.23
title: 'Window lifecycle: Cmd+W, Dock reopen, and window-state memory'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-20 23:48'
updated_date: '2026-08-21 14:34'
labels:
  - tauri
  - macos
milestone: m-0
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

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Worktree /home/dalton/Dev/nigel/wt-lifecycle, branch feat/desktop-lifecycle from main.
2. Extract the window build into build_main_window(handle), used by setup and by Reopen's rebuild fallback; the .min_inner_size(900.0, 700.0)-before-.build() source string stays intact for tests/window_size.rs (its path expectation updates if the builder moves files).
3. Run loop: .run(ctx) becomes .build(ctx) plus .run(handler). macOS keep-alive: RunEvent::ExitRequested with no explicit exit code gets api.prevent_exit() under cfg(target_os = macos); Windows and Linux never prevent, so close still exits there.
4. Close hides on macOS: CloseRequested on the main window prevents close and hides, so SPA state survives; RunEvent::Reopen with no visible window shows and focuses it, rebuilding via build_main_window only if the window is actually gone. Cmd+W itself arrives with TASK-33.22's menu; this task makes whatever closes the window behave.
5. Persistence, new src/window_state.rs: width/height/x/y in logical units, written to config-dir/window-state.json on CloseRequested and ExitRequested. No resize debounce — losing geometry to a crash is acceptable, and a timer is a moving part that buys little. nigel-core grows pub fn config_dir(), the one public-API addition (the keychain flag of 33.4 and the license key of 115.2 will want the same accessor; one public fn beats three private copies) — flagged for review.
6. Restore: pure fn clamp_restore(saved, monitors) applied in build_main_window — size floored at 900x700, position adjusted until the window intersects a visible monitor's work area; absent, corrupt or unparseable state falls through silently to 1200x820 centered (a UI-state file is not worth a dialog; the next clean close overwrites it). First launch unchanged.
7. Tests: clamp_restore units against fake monitor rects (offscreen, shrunken monitor, negative coords, multi-monitor); state-file round-trip and corrupt-file fallback under a temp dir; source-text tests pinning prevent_exit and the close-hides block inside cfg(target_os = macos). Gates: cargo fmt --check and cargo test in crates/nigel-desktop, plus nigel-core tests for the new pub fn.
8. Verifiable on Linux: persistence round-trip and clamp behavior. macOS-only list for the operator: keep-alive after last close, Dock reopen, Cmd+W once 33.22 lands.
<!-- SECTION:PLAN:END -->
