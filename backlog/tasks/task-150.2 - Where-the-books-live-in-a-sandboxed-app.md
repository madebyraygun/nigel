---
id: TASK-150.2
title: Where the books live in a sandboxed app
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - swift
  - macos
  - appstore
  - data-integrity
milestone: m-3
dependencies:
  - TASK-150.1
  - TASK-144.1
parent_task_id: TASK-150
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
config_dir() and default_data_dir() derive from dirs::home_dir(). In the sandbox that resolves inside the app container, so the store app would create new, empty books there and never see books the CLI keeps.

The shell owns the location instead. First run offers a folder, pre-pointed at ~/Documents/Nigel, or "Open existing books…". The shell stores an app-scoped security-scoped bookmark to the folder, not the .db file: SQLite needs its -wal, -shm and -journal siblings, and Nigel writes backups, pre-import snapshots, the uploads spool, filed document PDFs and the local-delivery outbox under the data directory. The shell starts access before NigelHost::new(data_dir) and passes the config directory explicitly; nigel-core learns nothing about bookmarks. A stale or missing bookmark re-prompts; it never falls back to a new database.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A fresh install puts books in the chosen folder, and they reopen after quit and reboot without a prompt
- [ ] #2 Opening an existing CLI books folder shows every transaction, rule, invoice, document PDF and backup
- [ ] #3 A moved or deleted folder produces a clear re-prompt, never a silent new database
- [ ] #4 No store-build code path creates a database the user did not choose
- [ ] #5 Settings shows where the books are, with Show in Finder
<!-- AC:END -->
