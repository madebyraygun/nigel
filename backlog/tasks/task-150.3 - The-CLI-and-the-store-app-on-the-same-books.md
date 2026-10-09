---
id: TASK-150.3
title: The CLI and the store app on the same books
status: To Do
assignee: []
created_date: '2026-10-07 18:46'
labels:
  - appstore
  - cli
  - docs
milestone: m-3
dependencies:
  - TASK-150.2
  - TASK-112
parent_task_id: TASK-150
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The store app cannot install the nigel CLI (single-bundle rule), and operators use the CLI and Claude skills against the same books. Cover how the CLI finds books the app chose (--data-dir or settings), what happens when the app is open and the CLI imports or serves (per TASK-112: works, or refuses with one sentence), and what macOS asks if a user keeps books inside the app container anyway. The app links to CLI install instructions; it does not install anything.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With the app open, nigel import and nigel serve on the same folder work or refuse with a sentence, per TASK-112
- [ ] #2 docs/desktop.md has a section on using the CLI with store-app books
<!-- AC:END -->
