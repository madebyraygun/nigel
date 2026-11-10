---
id: TASK-178
title: Bank sync setup saves settings.json outside the db_gate lock
status: To Do
assignee: []
created_date: '2026-11-10 02:07'
labels:
  - bug
  - web
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
server/routes/sync.rs reads, modifies and writes settings.json without holding `db_gate`, while a data-directory switch saves under its write lock. If setup reads settings, a data-dir switch saves, then setup saves, the old `data_dir` is written back and the next restart reopens the previous books. The window is milliseconds.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The setup save goes through the same gate as the data-directory switch
- [ ] #2 A test or a documented reason covers the interleaving
<!-- AC:END -->
