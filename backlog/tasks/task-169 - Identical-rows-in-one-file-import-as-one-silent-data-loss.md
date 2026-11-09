---
id: TASK-169
title: Identical rows in one file import as one (silent data loss)
status: To Do
assignee: []
created_date: '2026-11-09 22:22'
labels:
  - bug
  - importer
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`is_duplicate_row` checks each parsed row against the database, and rows are inserted one at a time, so a second row in the same file with the same (account, date, description, amount) matches the one just inserted and is skipped as a duplicate. Two $8.00 coffees on the same day become one, and the summary reports a skipped duplicate. Statements contain legitimately identical rows (transit fares, repeat purchases), and formats without a reference number cannot tell them apart.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A file with N identical rows imports N transactions on first import
- [ ] #2 Re-importing an overlapping file still skips rows already present, counting by multiplicity (file count vs database count per key)
- [ ] #3 Dry run reports the same counts the real import produces
- [ ] #4 Tests cover identical rows within one file, across two overlapping files, and mixed
<!-- AC:END -->
