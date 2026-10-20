---
id: TASK-109.17
title: 'Documents: include filed PDFs in backups'
status: Done
assignee: []
created_date: '2026-10-05 23:22'
updated_date: '2026-10-20 23:08'
labels:
  - documents
milestone: m-3
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nigel backup snapshots only the database; filed PDFs under <data_dir>/documents/ are not backed up, so a restore leaves documents whose version files are missing.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nigel backup includes <data_dir>/documents/ and a restore brings the files back
- [x] #2 A document whose version file is missing reports it clearly in show and the web detail
<!-- AC:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in PR #55 (9125202).
<!-- SECTION:FINAL_SUMMARY:END -->
