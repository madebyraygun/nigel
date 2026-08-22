---
id: TASK-7
title: 'Invoicing: Duplicate existing invoice'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-04-25 18:05'
updated_date: '2026-08-21 00:21'
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
- [ ] #1 nigel invoice duplicate <number> creates a fresh draft copying client, currency, notes, terms and line items, with a new number and token and no publish/void/Stripe state
- [ ] #2 A source with a due date duplicates preserving the issue-to-due offset in days; a source without one yields none
- [ ] #3 Any source state duplicates (draft, sent, paid, void); an archived client refuses the way create_invoice already refuses
- [ ] #4 The TUI invoice detail offers the action and lands on the new draft; the web invoice actions gain a Duplicate button behind POST /invoices/{number}/duplicate
- [ ] #5 Update test coverage
- [ ] #6 Create or update documentation
- [ ] #7 All linting checks pass
<!-- AC:END -->
