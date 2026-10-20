---
id: TASK-154.11
title: Bring the remaining screens onto the refresh
status: To Do
assignee: []
created_date: '2026-10-20 22:57'
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
