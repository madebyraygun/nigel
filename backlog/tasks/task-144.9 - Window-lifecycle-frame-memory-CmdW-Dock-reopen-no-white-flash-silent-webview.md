---
id: TASK-144.9
title: >-
  Window lifecycle: frame memory, Cmd+W, Dock reopen, no white flash, silent
  webview
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
dependencies:
  - TASK-144.3
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 2. Frame autosave, Cmd+W closes the window and Dock click reopens it, the window paints the page background before the page loads so there is no white flash on launch or resize, and WKWebView's context menu and keystroke beep are silenced. Absorbs TASK-33.23, 33.24 and 33.25.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Window size and position survive relaunch; Cmd+W closes and a Dock click reopens with state intact
- [ ] #2 No white flash on launch or resize in light or dark
- [ ] #3 No webview context menu; no beep on unhandled keystrokes; TASK-33.23, 33.24 and 33.25 are closed against this task
<!-- AC:END -->
