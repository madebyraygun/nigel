---
id: TASK-89
title: 'Web UI: register table should extend the full height of the window'
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
On the register screen the table's scroll container ends partway down the viewport and dead space runs below it (screenshot: 1872-row register, table stops ~70% down the window). The table should flex to fill the available height between the toolbar and the Net footer, with its own scrollbar.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The register table fills the vertical space between the toolbar and the bottom of the window at any viewport height
- [ ] #2 The Net summary row stays visible (pinned or at table end) and no dead space renders below the table
<!-- AC:END -->
