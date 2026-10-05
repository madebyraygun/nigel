---
id: TASK-109.17
title: 'Documents: include filed PDFs in backups'
status: To Do
assignee: []
created_date: '2026-10-05 23:22'
labels:
  - documents
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
- [ ] #1 nigel backup includes <data_dir>/documents/ and a restore brings the files back
- [ ] #2 A document whose version file is missing reports it clearly in show and the web detail
<!-- AC:END -->
