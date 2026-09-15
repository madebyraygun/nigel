---
id: TASK-144.15
title: Retire Tauri on macOS and describe the Swift shell in the docs
status: To Do
assignee: []
created_date: '2026-09-15 16:07'
labels:
  - swift
  - macos
  - docs
dependencies:
  - TASK-144.7
  - TASK-144.8
  - TASK-144.9
documentation:
  - docs/superpowers/specs/2026-09-14-swift-shell-design.md
parent_task_id: TASK-144
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 4. Once the shell reaches parity and the chrome is in, crates/nigel-desktop is gated to Windows and Linux (its CI keeps compiling on Linux; the macOS runner compiles the Swift shell instead). docs/desktop.md, docs/native-feel.md, docs/architecture.md and README describe the Swift shell as the macOS app, the Transport seam, the bridge contract and the dev loop; CLAUDE.md's pointer table gains the entry if a new doc is added. TASK-33's remaining macOS-only subtasks are closed or re-pointed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 cargo build of crates/nigel-desktop on macOS is no longer part of CI; Linux and Windows builds are unchanged
- [ ] #2 The docs describe the current state only, with no history of the Tauri shell on macOS
- [ ] #3 Every TASK-33 subtask this epic absorbed is closed with a note naming the task that did the work
<!-- AC:END -->
