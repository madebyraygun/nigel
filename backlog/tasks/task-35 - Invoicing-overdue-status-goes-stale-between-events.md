---
id: TASK-35
title: 'Invoicing: overdue status goes stale between events'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-08-06 19:14'
updated_date: '2026-08-20 14:36'
labels:
  - bug
  - invoicing
dependencies: []
references:
  - 'archived PR #172'
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Invoice status only refreshes on payment and publish events, so `invoice list` and `invoice show` can display `sent` for an invoice whose due date has passed. The aging report computes buckets independently and is unaffected, which means the two surfaces disagree about the same invoice.

Carried over from the review ledger of PR #172.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `invoice list` and `invoice show` report an invoice as overdue once its due date has passed, without requiring an intervening payment or publish event
- [ ] #2 Status shown by list/show agrees with the bucket the same invoice falls into on the aging report
- [ ] #3 Invoices with no due date are never marked overdue
- [ ] #4 A test covers an invoice whose due date passed with no events since publish
<!-- AC:END -->
