---
id: TASK-109.21
title: >-
  Documents screen: the client filter lists archived clients, and the drop zone
  asks for a statement
status: To Do
assignee: []
created_date: '2026-10-19 19:05'
labels:
  - documents
  - web
  - bug
milestone: m-3
dependencies: []
parent_task_id: TASK-109
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two bugs on the web Documents screen.

1. The Client filter above the list offers every client, archived ones included. The filing form's picker already narrows to active clients (`this.clients.filter((client) => client.archivedAt === null)` in web/apps/app/src/screens/documents.ts), but the filter maps over `this.clients` directly.

2. The filing drop zone reads "Drop a statement here, or choose a file": wc-dropzone hardcodes the bank-import prompt. A document is a PDF, not a statement.

Note for 1: an archived client can still have documents filed before archiving. Narrowing the filter to active clients makes those reachable only through the All clients list; if that matters, list archived clients that have documents in a separate group rather than dropping them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Documents screen's Client filter lists active clients only
- [ ] #2 wc-dropzone takes its prompt as a property, with a preview state for it; the Documents screen asks for a PDF ('Drop a PDF here, or choose a file') and bank imports keep their wording
- [ ] #3 Tests cover both, and describePreviewA11y passes with zero violations
<!-- AC:END -->
