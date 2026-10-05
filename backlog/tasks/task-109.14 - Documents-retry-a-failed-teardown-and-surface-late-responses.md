---
id: TASK-109.14
title: 'Documents: retry a failed teardown and surface late responses'
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
When closing the manifest or republishing pages fails after revise, withdraw, a manual verb or sync, the warning is the only trace and nothing retries it; sync only checks sent and changes_requested documents, so a response the Worker accepted on a now-terminal document is never surfaced. Add a republish/retry path and report late responses (docs/superpowers/specs/2026-10-05-documents-signing-design.md).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 nigel document republish <id> (and an API route) re-closes the manifest and re-renders every page for the document's current state
- [ ] #2 Sync reports a response found on a terminal document as a refused line instead of ignoring it
- [ ] #3 The web UI offers republish when the last action returned teardown warnings
<!-- AC:END -->
