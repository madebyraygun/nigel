---
id: TASK-154.11
title: Bring the remaining screens onto the refresh
status: In Progress
assignee: []
created_date: '2026-10-20 22:57'
updated_date: '2026-10-24 02:02'
labels:
  - web
  - design
milestone: m-3
dependencies:
  - TASK-154.1
  - TASK-154.2
references:
  - docs/design/app-refresh/README.md
parent_task_id: TASK-154
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The handoff covers Dashboard, Register, Invoices, Settings and Setup. Bring Review, Reports, Import, Reconcile, Undo, Clients, Documents, Accounts, Categories and Rules onto the same tokens, page title treatment, cards, chips, pills, buttons and focus rings, so no screen keeps the old look.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each listed screen uses the refresh's title row, cards, buttons, chips and focus rings in light and dark
- [ ] #2 Every new or changed @nigel/ui component has a preview covering its states in light and dark, and describePreviewA11y passes with zero violations
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Part 1 shipped in PR #70: Clients, Accounts, Categories and Rules on the refresh; manager dialogs fit and animate; a shared stacked-table sheet (stacked-table.ts) makes the manager, invoice and document tables stack into cards below 720px. Remaining screens follow in later parts.
<!-- SECTION:NOTES:END -->
