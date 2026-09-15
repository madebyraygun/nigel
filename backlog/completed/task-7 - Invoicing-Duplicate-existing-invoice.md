---
id: TASK-7
title: 'Invoicing: Duplicate existing invoice'
status: Done
assignee:
  - '@claude'
created_date: '2026-04-25 18:05'
updated_date: '2026-09-15 16:06'
labels: []
milestone: m-0
dependencies: []
references:
  - 'archived issue #30'
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
(no description on original issue)

---
*Migrated from archived GitHub issue #30*
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 nigel invoice duplicate <number> creates a fresh draft copying client, currency, notes, terms and line items, with a new number and token and no publish/void/Stripe state
- [x] #2 A source with a due date duplicates preserving the issue-to-due offset in days; a source without one yields none
- [x] #3 Any source state duplicates (draft, sent, paid, void); an archived client refuses the way create_invoice already refuses
- [x] #4 The TUI invoice detail offers the action and lands on the new draft; the web invoice actions gain a Duplicate button behind POST /invoices/{number}/duplicate
- [x] #5 Update test coverage
- [x] #6 Create or update documentation
- [x] #7 All linting checks pass
<!-- AC:END -->
