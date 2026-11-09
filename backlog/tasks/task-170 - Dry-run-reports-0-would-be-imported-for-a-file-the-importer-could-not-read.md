---
id: TASK-170
title: Dry run reports 0 would be imported for a file the importer could not read
status: To Do
assignee: []
created_date: '2026-11-09 22:22'
labels:
  - bug
  - importer
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When no importer detects a file, `get_for_file` falls back to the first importer for the account type, whose parser reads zero rows. A real import refuses with `EmptyImport`, but a dry run is exempt and prints "0 would be imported, 0 duplicates", which reads like a clean preview of an already-imported file. A `.qbo` dry run against a checking account produces exactly this.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A dry run that parses zero rows says so explicitly, naming the format tried and the first rejection reasons, as `EmptyImport` does
- [ ] #2 When detection fails and the importer is chosen by fallback, both dry run and real import say the format was assumed, not detected
- [ ] #3 The web import preview surfaces the same warning
<!-- AC:END -->
