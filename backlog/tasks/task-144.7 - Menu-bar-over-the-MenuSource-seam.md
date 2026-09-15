---
id: TASK-144.7
title: Menu bar over the MenuSource seam
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
  - ui
dependencies:
  - TASK-144.5
  - TASK-33.22
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 2. The bar TASK-33.22 specifies, verbatim: app menu with About and Settings (Cmd+,), File with Import Statement... (Cmd+O) and New Invoice (Cmd+N), Edit built from the standard items (load-bearing for clipboard chords in WKWebView) plus Find (Cmd+F), View with Cmd+1 through Cmd+9 over the posted registry and Toggle Sidebar, Window as the NSApp windows menu, Help. Custom items emit menu-command with the same ids 33.22's MenuSource defines, so that seam is built once and serves both shells. Depends on 33.22 landing the SPA half.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The full bar is present with the standard items and accelerators; clipboard, undo and select-all chords work in every text field
- [ ] #2 Cmd+1 through Cmd+9 navigate in registry order; Find focuses the register filter; Import Statement opens the open panel; New Invoice opens a new invoice
- [ ] #3 Menu ids are the ones MenuSource defines; no id is invented in the shell
<!-- AC:END -->
