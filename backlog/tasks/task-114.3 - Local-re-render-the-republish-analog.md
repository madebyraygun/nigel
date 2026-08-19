---
id: TASK-114.3
title: 'Local re-render: the republish analog'
status: To Do
assignee: []
created_date: '2026-08-17 13:27'
labels:
  - invoicing
  - documents
dependencies: []
parent_task_id: TASK-114
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The state changes that republish a hosted page re-render the outbox artifacts instead, under the same best-effort rule.

- A recorded payment that would republish an invoice page (task 64's machinery) re-renders `outbox/i/<token>/…`; a document acceptance or countersign re-renders the stamped page; withdraw and void replace the local page with the withdrawn/voided notice, PDF left in place — `void_invoice_with_teardown`'s shape pointed at disk.
- Best-effort, warnings as data: a failed re-render never loses the recorded state (the republish precedent verbatim).
- One seam serves both modes: republish and re-render are the same decision — what does this state change do to the published artifact — routed through the active `AssetPublisher`, not a parallel code path.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each state change that republishes in hosted mode re-renders the outbox in local mode, through the same seam, pinned by shared tests
- [ ] #2 Withdraw and void replace the local page with a notice and leave the PDF, warnings reported as data
- [ ] #3 A failed re-render is a warning and never loses recorded state
<!-- AC:END -->
