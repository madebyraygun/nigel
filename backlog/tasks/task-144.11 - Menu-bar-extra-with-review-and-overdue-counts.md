---
id: TASK-144.11
title: Menu bar extra with review and overdue counts
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
Phase 3. A MenuBarExtra showing the review queue count and the overdue invoice count read through the router, with Open Nigel, Import Statement..., New Invoice and Review actions. Refreshed on activation and after any 2xx non-GET response passes through the scheme handler, which needs no SPA cooperation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The item shows both counts and refreshes after a mutation in the app and on activation
- [ ] #2 Each action opens the main window on the right screen, importing through the open panel
- [ ] #3 The extra can be turned off in the app's settings
<!-- AC:END -->
