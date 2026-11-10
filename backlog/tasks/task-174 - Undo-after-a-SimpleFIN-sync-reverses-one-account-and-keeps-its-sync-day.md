---
id: TASK-174
title: Undo after a SimpleFIN sync reverses one account and keeps its sync day
status: To Do
assignee: []
created_date: '2026-11-10 02:07'
labels:
  - bug
  - importer
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A sync of N mapped accounts writes N `imports` rows, but `undo` removes only the last one, and it leaves that account's `simplefin_accounts.last_synced` in place. After undoing a first sync that pulled weeks of rows, the next sync starts a week before the recorded sync day and the older rows stay missing unless the user knows to pass `--since`. docs/importers.md says undo "treats it like a file", which overstates it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Undoing a sync removes every import that one sync run created, across all its accounts
- [ ] #2 Undo restores each affected account's `last_synced` to its value before that sync, so the next sync re-fetches what was removed
- [ ] #3 docs/importers.md describes what undo does to a sync
- [ ] #4 Tests cover undoing a multi-account sync and the following sync window
<!-- AC:END -->
