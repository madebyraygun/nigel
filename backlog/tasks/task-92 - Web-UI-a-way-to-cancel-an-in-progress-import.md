---
id: TASK-92
title: 'Web UI: a way to cancel an in-progress import'
status: To Do
assignee: []
created_date: '2026-08-12 17:50'
labels:
  - web
  - ui
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The import flow (choose file, preview, confirm) has no cancel affordance. A user who previews the wrong file or picks the wrong account should be able to abandon the import cleanly: clear the chosen file, preview state and any spooled upload, and return the screen to its initial state without confirming.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A cancel control abandons the current import at any pre-confirm stage and resets the screen
- [ ] #2 Cancelling after a preview leaves no orphaned upload in the server spool beyond the existing purge
<!-- AC:END -->
