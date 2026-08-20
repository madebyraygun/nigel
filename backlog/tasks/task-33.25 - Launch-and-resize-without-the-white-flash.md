---
id: TASK-33.25
title: Launch and resize without the white flash
status: To Do
assignee: []
created_date: '2026-08-20 23:49'
labels:
  - tauri
  - macos
dependencies: []
parent_task_id: TASK-33
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The window paints white before the SPA's first frame and shows white at the edges when a resize outruns the webview — the webview's default background peeking out from behind the page. A native window is never a different color than its content.

Two shell-side changes. Give the window a background color matching the theme bg, resolving dark mode at build time the way the SPA's pre-paint script does, so a dark-mode launch does not flash light — the OS appearance is the shell's best signal, with the stored in-app override applied by the pre-paint script an instant later. And create the window hidden, showing it once the frontend signals ready, so first paint is the app rather than a blank sheet. Show-on-ready trades a flash for a beat of nothing; the beat has to stay short enough to read as instant.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Launching in light and in dark shows no wrong-color flash before first paint
- [ ] #2 Fast resizes show the theme background at the edges, never white
- [ ] #3 The window appears promptly — perceived launch is not slower
- [ ] #4 Windows and Linux are unaffected or equally improved; no @nigel/ui change
<!-- AC:END -->
