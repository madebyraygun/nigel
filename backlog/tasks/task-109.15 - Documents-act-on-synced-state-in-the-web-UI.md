---
id: TASK-109.15
title: 'Documents: act on synced state in the web UI'
status: To Do
assignee: []
created_date: '2026-10-05 23:22'
updated_date: '2026-10-07 18:44'
labels:
  - documents
milestone: m-3
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
nigel serve skips the launch sync, so Revise, Withdraw or a manual verb in the web UI can close the manifest over an online response that was never recorded, and nobody is told. Sync the one document before those actions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Revise, withdraw, accept, request-changes, decline and countersign from the API first sync that document's responses
- [ ] #2 A response found that way is recorded or refused and shown in the action's result
<!-- AC:END -->
