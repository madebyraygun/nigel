---
id: TASK-144.4
title: >-
  Bridge: __NIGEL_SHELL__ invoke/listen, native panels, drops, and parity with
  the Tauri shell
status: To Do
assignee: []
created_date: '2026-09-15 16:06'
labels:
  - swift
  - macos
  - ui
dependencies:
  - TASK-144.3
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1. A WKUserScript at document start defines window.__NIGEL_SHELL__ = { invoke, listen } with the shapes of window.__TAURI__.core.invoke and .event.listen: invoke posts to a WKScriptMessageHandlerWithReply and resolves with its reply; listen subscribes to an in-page emitter the shell drives with evaluateJavaScript. createApiClient looks for __NIGEL_SHELL__ beside __TAURI__ and reuses DesktopApiClient as is. The shell answers save_export (NSSavePanel), stage_import, pick_import_file (NSOpenPanel) and pick_directory, and emits the four drag events for file drops on the window, routed through stage_file and the existing ImportSource seam. CFBundleDocumentTypes for CSV and XLSX so a drop on the Dock icon or Finder's Open With lands in the import flow. At the end of this task every screen works as it does under Tauri.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 createApiClient returns a DesktopApiClient when __NIGEL_SHELL__ is present and both members are functions; the browser build is unchanged
- [ ] #2 Imports work by open panel, by drop on the window, by drop on the Dock icon and by Finder's Open With; exports save through NSSavePanel; the invoice PDF preview renders
- [ ] #3 Every screen passes the same manual parity checklist the Tauri shell passes, recorded in the PR
- [ ] #4 Bridge codec and message handling are covered by Swift tests; web tests cover the detection and a fake shell
<!-- AC:END -->
