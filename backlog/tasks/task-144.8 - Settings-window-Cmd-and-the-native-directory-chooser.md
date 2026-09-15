---
id: TASK-144.8
title: 'Settings window (Cmd+,) and the native directory chooser'
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
  - ui
dependencies:
  - TASK-144.4
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 2. A second WKWebView at nigel://localhost/#/settings?shell=native in its own fixed-size window, opened from the app menu, closed with Cmd+W, reused rather than stacked. Data Directory uses NSOpenPanel in directory mode through the pick_directory command. Absorbs TASK-33.14 and 33.19.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cmd+, opens settings in its own window; reopening focuses the existing window; closing it leaves the main window as it was
- [ ] #2 Changing Data Directory opens a native directory chooser and the setting round-trips through the existing settings API
- [ ] #3 The browser build still reaches settings as a route; TASK-33.14 and 33.19 are closed against this task
<!-- AC:END -->
