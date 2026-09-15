---
id: TASK-144.10
title: 'Command palette (Cmd+K) over screens, actions and records'
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
  - ui
dependencies:
  - TASK-144.6
  - TASK-144.7
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 3. A native panel with three sources: screens from the posted registry, actions from the menu command table, and records searched through the in-process router (clients, invoices, and the register with q). Selection navigates by hash with params, which hash-route.ts already parses. Ranking and the source model live in NigelKit so they are testable without a window.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Cmd+K opens the palette from any screen; Esc closes it; arrows and Return select
- [ ] #2 Screens, actions and records rank in one list; choosing a record deep-links to it by hash with params
- [ ] #3 Ranking and source decoding are covered by Swift tests in NigelKit
<!-- AC:END -->
