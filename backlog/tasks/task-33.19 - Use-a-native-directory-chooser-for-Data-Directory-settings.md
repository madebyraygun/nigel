---
id: TASK-33.19
title: Use a native directory chooser for Data Directory settings
status: To Do
assignee: []
created_date: '2026-08-20 19:01'
labels:
  - tauri
  - ui
dependencies: []
parent_task_id: TASK-33
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Settings screen's Data Directory panel currently asks for a filesystem path in a text field. In the local desktop app, choosing a directory is shell-owned behavior and should use the operating system's native directory chooser instead of requiring someone to type or paste a path. The selected path should feed the existing confirmation and POST /api/settings/data-dir flow, preserving its validation, locking, error handling, and full-app reload. Browser and remote-server clients must keep the text field because a browser chooser selects something on the user's machine, not a path the server can open.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In the local desktop app, the Data Directory panel offers a native directory chooser instead of an editable path field
- [ ] #2 Choosing a directory passes its path into the existing switch confirmation and data-directory API flow; cancelling the chooser changes nothing and shows no error
- [ ] #3 The chooser accepts directories only, while missing or invalid Nigel databases are still rejected by the existing backend validation and surfaced in the panel
- [ ] #4 Browser and remote-server clients retain the typed-path control, with the settings screen branching through the API-client capability seam rather than reading Tauri globals
<!-- AC:END -->
