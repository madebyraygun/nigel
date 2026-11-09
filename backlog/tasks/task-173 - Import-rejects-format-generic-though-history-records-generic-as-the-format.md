---
id: TASK-173
title: Import rejects --format generic though history records generic as the format
status: To Do
assignee: []
created_date: '2026-11-09 22:22'
labels:
  - cli
  - importer
dependencies: []
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
An inline column mapping (`--date-col/--desc-col/--amount-col`) is recorded in import history under the format `generic`, but `--format generic` fails with "Unknown format: 'generic'", so the obvious way to ask for it does not work.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `--format generic` is accepted with the column flags, and refused with a message naming the required flags without them
- [ ] #2 `import --help` documents generic import
<!-- AC:END -->
